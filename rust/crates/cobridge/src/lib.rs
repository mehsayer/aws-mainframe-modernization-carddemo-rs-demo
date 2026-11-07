//! CICS bridge: runs the ORIGINAL CardDemo online programs (COBOL, compiled with GnuCOBOL) on the same
//! CICS runtime as the Rust translation (screens, files, transactions, TD queue, data store).
//!
//! `rust/cobridge/cicsprep.py` translates every `EXEC CICS` into
//! `CALL EIB-CDCICS USING DFHEIBLK BY CONTENT 'Dn' <descriptor pieces> <arguments>`.
//! `EIB-CDCICS` is a procedure pointer this module stores in the EIB, so it needs no exported symbols.
//! The descriptor names the command and its options (`READ DATASET=1 RIDFLD=2 ... RESP`); `=k` options
//! take argument k, which [`cdcics`] reads through GnuCOBOL's C API (data address and size).
//!
//! Return codes to the translated program: 0 = continue; 1..n = GO TO the n-th HANDLE CONDITION /
//! HANDLE ABEND label; 99 = the task leaves the program (XCTL, RETURN, ABEND).

use cics::bms::MapDef;
use cics::{Aid, AttrByte, Color, Cr, Program, Region, Task, Transfer};
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CString};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use vsam::{Browse, Resp};

#[cfg(unix)]
use libc::{dlerror, dlopen, dlsym, RTLD_GLOBAL, RTLD_NOW};

mod sysout;
pub use sysout::{send_to_sysout, set_sysout, sysout_path, write_sysout};

/// The GnuCOBOL runtime functions the bridge uses. libcob is loaded at run time (dlopen), so nothing in
/// the workspace needs GnuCOBOL to build; only running the COBOL programs does.
struct Cob {
    init: unsafe extern "C" fn(c_int, *mut *mut c_char),
    resolve: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    call: unsafe extern "C" fn(*const c_char, c_int, *mut *mut c_void) -> c_int,
    cancel: unsafe extern "C" fn(*const c_char),
    param_size: unsafe extern "C" fn(c_int) -> c_int,
    param_data: unsafe extern "C" fn(c_int) -> *mut c_void,
    get_s64: unsafe extern "C" fn(c_int) -> i64,
    put_s64: unsafe extern "C" fn(c_int, i64),
    param_constant: unsafe extern "C" fn(c_int) -> c_int,
}

static COB: OnceLock<Result<Cob, String>> = OnceLock::new();

#[cfg(unix)]
fn last_dl_error() -> String {
    let e = unsafe { dlerror() };
    if e.is_null() {
        "unknown error".into()
    } else {
        unsafe { std::ffi::CStr::from_ptr(e) }.to_string_lossy().into_owned()
    }
}

/// libcob: exactly `CARDDEMO_LIBCOB` when it is set (no fallback: it must match the cobc that built the
/// modules), else the usual shared-library names. The loader's own message is reported on failure.
#[cfg(unix)]
fn load_cob() -> Result<Cob, String> {
    let explicit = std::env::var("CARDDEMO_LIBCOB").ok().filter(|v| !v.is_empty());
    let names: Vec<String> = match &explicit {
        Some(p) => vec![p.clone()],
        None => ["libcob.so.4", "libcob.so"].map(String::from).to_vec(),
    };
    let mut handle = std::ptr::null_mut();
    let mut errors = Vec::new();
    for n in &names {
        let c = CString::new(n.as_str()).map_err(|e| e.to_string())?;
        handle = unsafe { dlopen(c.as_ptr(), RTLD_NOW | RTLD_GLOBAL) };
        if !handle.is_null() {
            break;
        }
        errors.push(last_dl_error());
    }
    if handle.is_null() {
        return Err(match explicit {
            Some(p) => format!("cannot load CARDDEMO_LIBCOB={p}: {}", errors.join("; ")),
            None => format!("cannot load the GnuCOBOL runtime (libcob; install GnuCOBOL 3.x or set CARDDEMO_LIBCOB): {}", errors.join("; ")),
        });
    }
    let sym = |n: &str| -> Result<*mut c_void, String> {
        let c = CString::new(n).unwrap();
        let p = unsafe { dlsym(handle, c.as_ptr()) };
        if p.is_null() {
            Err(format!("libcob has no {n} (GnuCOBOL 3.x needed)"))
        } else {
            Ok(p)
        }
    };
    unsafe {
        Ok(Cob {
            init: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(c_int, *mut *mut c_char)>(sym("cob_init")?),
            resolve: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*const c_char) -> *mut c_void>(sym("cob_resolve")?),
            call: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*const c_char, c_int, *mut *mut c_void) -> c_int>(sym("cob_call")?),
            cancel: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*const c_char)>(sym("cob_cancel")?),
            param_size: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(c_int) -> c_int>(sym("cob_get_param_size")?),
            param_data: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(c_int) -> *mut c_void>(sym("cob_get_param_data")?),
            get_s64: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(c_int) -> i64>(sym("cob_get_s64_param")?),
            put_s64: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(c_int, i64)>(sym("cob_put_s64_param")?),
            param_constant: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(c_int) -> c_int>(sym("cob_get_param_constant")?),
        })
    }
}

/// GnuCOBOL's runtime is only loaded on Unix systems (the bridge runs on Linux; Windows builds without it).
#[cfg(not(unix))]
fn load_cob() -> Result<Cob, String> {
    Err("the COBOL programs run on Linux only (GnuCOBOL's libcob)".into())
}

fn cob() -> &'static Cob {
    COB.get_or_init(load_cob).as_ref().expect("libcob checked by cobridge::check()")
}

/// True while a COBOL program (or libcob's initialisation) runs.
static IN_COBOL: AtomicBool = AtomicBool::new(false);

/// True while a COBOL program, or libcob's initialisation, is running. An exit handler that finds it true
/// knows that libcob is ending the process (runtime error, STOP RUN) and that its message is in the
/// sysout file ([`sysout_path`]).
pub fn in_cobol() -> bool {
    IN_COBOL.load(Ordering::SeqCst)
}

/// Run `f` (a libcob call) with [`in_cobol`] true.
fn in_cobol_call<R>(f: impl FnOnce() -> R) -> R {
    IN_COBOL.store(true, Ordering::SeqCst);
    let r = f();
    IN_COBOL.store(false, Ordering::SeqCst);
    r
}

const RC_TRANSFER: c_int = 99;

// DFHEIBLK layout (rust/cobridge/copy/DFHEIBLK.cpy)
const EIB_TIME: usize = 0;
const EIB_DATE: usize = 4;
const EIB_TRNID: usize = 8;
const EIB_TRMID: usize = 16;
const EIB_CALEN: usize = 24;
const EIB_AID: usize = 26;
const EIB_RESP: usize = 59;
const EIB_RESP2: usize = 63;
const EIB_CDCICS: usize = 67;
const EIB_LEN: usize = 75;

/// Directory of the compiled COBOL modules: a non-empty `CARDDEMO_COBOL_MODULES` made absolute against the
/// current directory (symlinks and `..` kept, as the OS will resolve them; as given if that fails), else
/// the build output. Same rule as `modules_dir()` in cobridge/cicsprep.py.
pub fn modules_dir() -> PathBuf {
    match std::env::var_os("CARDDEMO_COBOL_MODULES").filter(|v| !v.is_empty()) {
        Some(v) => {
            let given = PathBuf::from(v);
            std::path::absolute(&given).unwrap_or(given)
        }
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../cobridge/build/modules"),
    }
}

/// True when the COBOL modules have been built (`python3 rust/cobridge/cicsprep.py`).
pub fn available() -> bool {
    modules_dir().join("COSGN00C.so").exists()
}

/// Everything needed to run the COBOL programs: the modules and the GnuCOBOL runtime.
pub fn check() -> Result<(), String> {
    if !available() {
        return Err(format!(
            "COBOL modules not built in {}: run  python3 rust/cobridge/cicsprep.py  (needs GnuCOBOL)",
            modules_dir().display()
        ));
    }
    COB.get_or_init(load_cob).as_ref().map(|_| ()).map_err(Clone::clone)
}

/// The outcome of the one initialisation of the COBOL runtime (see [`start`]).
static INIT: OnceLock<Result<(), String>> = OnceLock::new();

/// Initialise the COBOL runtime, once per process: [`check`], then `cob_init`. The first program call does
/// it if nobody did before; its outcome is kept, so a failure is reported the same way to every caller.
/// libcob installs its signal handlers here: a UI that adds its own (to restore the terminal) calls this
/// first, so that its handlers run first and then pass the signal on to libcob's. libcob may end the
/// process here (a broken runtime configuration): register an exit handler before ([`in_cobol`] is true).
pub fn start() -> Result<(), String> {
    INIT.get_or_init(|| {
        std::env::set_var("COB_LIBRARY_PATH", modules_dir());
        // A fixed test clock (CARDDEMO_NOW) is passed on to FUNCTION CURRENT-DATE. GnuCOBOL treats
        // COB_CURRENT_DATE as a frozen clock, so it is only set when the Rust side is frozen too.
        if let Some(now) = cobol::clock::fixed_now() {
            std::env::set_var("COB_CURRENT_DATE", now.format("%Y/%m/%d %H:%M:%S").to_string());
        }
        check()?;
        #[cfg(debug_assertions)]
        test_seam::from_env();
        in_cobol_call(|| unsafe { (cob().init)(0, std::ptr::null_mut()) });
        Ok(())
    })
    .clone()
}

/// A region whose programs are the original COBOL programs (same transactions, maps, data store).
pub fn region(vsam: vsam::Vsam) -> Region {
    let mut r = online::region(vsam);
    r.programs = r.programs.keys().map(|k| (*k, run as Program)).collect();
    r
}

/// Per-program-invocation state of the bridge.
struct Ctx {
    task: *mut Task<'static>,
    transfer: Option<Transfer>,
    browses: HashMap<String, Browse>,
    /// HANDLE CONDITION: condition name -> label number
    handle: HashMap<String, c_int>,
    /// HANDLE ABEND LABEL
    abend_label: Option<c_int>,
}

thread_local! {
    static CTX: Cell<*mut Ctx> = const { Cell::new(std::ptr::null_mut()) };
}

fn aid_byte(a: Aid) -> u8 {
    match a {
        Aid::Enter => 0x7D,
        Aid::Clear => 0x6D,
        Aid::Pa1 => 0x6C,
        Aid::Pa2 => 0x6E,
        Aid::Pf(n @ 1..=9) => 0xF0 + n,
        Aid::Pf(10) => 0x7A,
        Aid::Pf(11) => 0x7B,
        Aid::Pf(12) => 0x7C,
        Aid::Pf(n @ 13..=21) => 0xC0 + (n - 12),
        Aid::Pf(22) => 0x4A,
        Aid::Pf(23) => 0x4B,
        Aid::Pf(24) => 0x4C,
        Aid::Pf(_) => 0x7D,
    }
}

/// Packed decimal S9(7) (4 bytes) of a 7-digit value.
fn pack7(v: u32) -> [u8; 4] {
    let d = format!("{:07}", v % 10_000_000).into_bytes();
    let n = |i: usize| d[i] - b'0';
    [n(0) << 4 | n(1), n(2) << 4 | n(3), n(4) << 4 | n(5), n(6) << 4 | 0x0C]
}

/// Run one original COBOL program for the current task (Region calls this like a Rust program).
pub fn run(t: &mut Task) -> Cr {
    if let Err(e) = start() {
        // the program cannot be loaded, as CICS reports it
        t.log.push(format!("cobridge: {e}"));
        return Err(Transfer::Abend("APCT".into()));
    }
    let name = CString::new(t.program.trim()).expect("program name");
    if in_cobol_call(|| unsafe { (cob().resolve)(name.as_ptr()) }).is_null() {
        return Err(Transfer::Abend("APCT".into()));
    }
    let now = cobol::clock::now();
    let mut eib = vec![0u8; EIB_LEN];
    eib[EIB_TIME..EIB_TIME + 4].copy_from_slice(&pack7(now.format("%H%M%S").to_string().parse().unwrap_or(0)));
    let cyyddd = format!("{}{}", (now.format("%Y").to_string().parse::<u32>().unwrap_or(1900) - 1900) / 100, now.format("%y%j"));
    eib[EIB_DATE..EIB_DATE + 4].copy_from_slice(&pack7(cyyddd.parse().unwrap_or(0)));
    eib[EIB_TRNID..EIB_TRNID + 4].fill(b' ');
    eib[EIB_TRMID..EIB_TRMID + 4].copy_from_slice(b"T001");
    eib[EIB_CALEN..EIB_CALEN + 2].copy_from_slice(&(t.commarea.len() as i16).to_be_bytes());
    eib[EIB_AID] = aid_byte(t.eibaid);
    let entry: extern "C" fn() -> c_int = cdcics;
    eib[EIB_CDCICS..EIB_CDCICS + 8].copy_from_slice(&(entry as usize).to_ne_bytes());
    // The commarea is handed over in a zero-filled area of the CICS maximum (32 KB), EIBCALEN keeps the
    // real length. Several programs copy their program area from beyond EIBCALEN (KD-16): on CICS that
    // reads whatever storage follows; here it reads zeros instead of memory outside the buffer.
    let mut ca = vec![0u8; 32 * 1024];
    ca[..t.commarea.len()].copy_from_slice(&t.commarea);

    let mut ctx = Ctx {
        task: (t as *mut Task).cast::<Task<'static>>(),
        transfer: None,
        browses: HashMap::new(),
        handle: HashMap::new(),
        abend_label: None,
    };
    let prev = CTX.with(|c| c.replace(&mut ctx));
    let mut args = [eib.as_mut_ptr() as *mut c_void, ca.as_mut_ptr() as *mut c_void];
    in_cobol_call(|| unsafe { (cob().call)(name.as_ptr(), 2, args.as_mut_ptr()) });
    // CICS gives every task a fresh copy of WORKING-STORAGE; a CALLed GnuCOBOL program keeps its state
    // (and any PERFORM return addresses left by a HANDLE CONDITION GO TO) until it is cancelled
    in_cobol_call(|| unsafe { (cob().cancel)(name.as_ptr()) });
    sysout::flush();
    CTX.with(|c| c.set(prev));
    match ctx.transfer.take() {
        Some(tr) => Err(tr),
        // GOBACK without EXEC CICS RETURN ends the task like RETURN
        None => Ok(()),
    }
}

// ---------------------------------------------------------------------------- argument access

unsafe fn param_bytes<'x>(i: c_int) -> &'x mut [u8] {
    let p = (cob().param_data)(i) as *mut u8;
    let n = (cob().param_size)(i).max(0) as usize;
    if p.is_null() {
        return &mut [];
    }
    std::slice::from_raw_parts_mut(p, n)
}

/// One parsed command descriptor.
struct Cmd {
    words: Vec<String>,
    /// option -> parameter number (GnuCOBOL numbering)
    args: HashMap<String, c_int>,
    labels: HashMap<String, c_int>,
}

impl Cmd {
    fn parse(desc: &str, base: c_int) -> Cmd {
        let mut c = Cmd { words: Vec::new(), args: HashMap::new(), labels: HashMap::new() };
        for w in desc.split_whitespace() {
            if let Some((k, v)) = w.split_once('=') {
                c.args.insert(k.to_string(), base + v.parse::<c_int>().unwrap_or(1) - 1);
            } else if let Some((k, v)) = w.split_once(':') {
                c.labels.insert(k.to_string(), v.parse().unwrap_or(0));
            } else {
                c.words.push(w.to_string());
            }
        }
        c
    }
    fn has(&self, w: &str) -> bool {
        self.words.iter().any(|x| x == w)
    }
    fn arg(&self, o: &str) -> Option<c_int> {
        self.args.get(o).copied()
    }
    fn bytes(&self, o: &str) -> &'static mut [u8] {
        match self.arg(o) {
            Some(i) => unsafe { param_bytes(i) },
            None => &mut [],
        }
    }
    fn text(&self, o: &str) -> String {
        cobol::text::s(self.bytes(o)).trim().to_string()
    }
    fn num(&self, o: &str) -> Option<i64> {
        self.arg(o).map(|i| unsafe { (cob().get_s64)(i) })
    }
    fn put_num(&self, o: &str, v: i64) {
        // LENGTH OF ... is passed BY CONTENT (a constant): nothing to give back
        if let Some(i) = self.arg(o) {
            if unsafe { (cob().param_constant)(i) } == 0 {
                unsafe { (cob().put_s64)(i, v) }
            }
        }
    }
    /// FROM area, LENGTH(...) bytes long when given
    fn from(&self) -> &'static [u8] {
        let b = self.bytes("FROM");
        let n = self.num("LENGTH").map_or(b.len(), |l| (l.max(0) as usize).min(b.len()));
        &b[..n]
    }
    fn file(&self) -> String {
        let f = self.text("DATASET");
        if f.is_empty() {
            self.text("FILE")
        } else {
            f
        }
    }
    fn key(&self) -> Vec<u8> {
        let k = self.bytes("RIDFLD");
        let n = self.num("KEYLENGTH").map_or(k.len(), |l| (l.max(0) as usize).min(k.len()));
        k[..n].to_vec()
    }
}

enum Out {
    Ok,
    /// CICS condition: (EIBRESP, EIBRESP2)
    Cond(i32, i32),
    Transfer(Transfer),
}

fn cond(e: Resp) -> Out {
    Out::Cond(e.code(), e.code2())
}

const PGMIDERR: i32 = 27;
const INVREQ: i32 = 16;

fn cond_name(resp: i32) -> &'static str {
    match resp {
        12 => "FILENOTFOUND",
        13 => "NOTFND",
        14 => "DUPREC",
        15 => "DUPKEY",
        16 => "INVREQ",
        17 => "IOERR",
        19 => "NOTOPEN",
        20 => "ENDFILE",
        22 => "LENGERR",
        27 => "PGMIDERR",
        36 => "MAPFAIL",
        _ => "ERROR",
    }
}

/// Abend code CICS uses when a condition is neither tested (RESP/NOHANDLE) nor handled.
fn cond_abend(resp: i32) -> String {
    match resp {
        12 => "AEIL",
        13 => "AEIM",
        14 => "AEIN",
        15 => "AEIO",
        16 => "AEIP",
        17 => "AEIQ",
        19 => "AEIS",
        22 => "AEIV",
        27 => "AEI0",
        36 => "AEI9",
        _ => "AEIA",
    }
    .to_string()
}

thread_local! {
    /// True while this thread runs a `CDCICS` call (see [`in_cdcics`]).
    static IN_CDCICS: Cell<bool> = const { Cell::new(false) };
    /// Panics of the current `CDCICS` call not yet caught (see [`note_panic`]).
    static PANICS: Cell<u32> = const { Cell::new(0) };
}

/// True while the current thread is inside an EXEC CICS command of a COBOL program.
pub fn in_cdcics() -> bool {
    IN_CDCICS.with(|c| c.get())
}

/// To be called by the process's panic hook, exactly once per panic (it counts them): true if the bridge
/// will catch this panic, i.e. it happens inside an EXEC CICS command and is the only one in flight there.
/// The task then ends with ASRA while the UI keeps running, so a TUI's hook must leave the terminal alone
/// (and may put the message into the sysout file, [`write_sysout`]). A second panic before the first is
/// caught (a destructor panicking during the unwind) aborts the process: false then. Only a hook can count
/// them, as only a hook sees a panic when it starts; the bridge installs no panic hook itself.
pub fn note_panic() -> bool {
    if !in_cdcics() {
        return false;
    }
    PANICS.with(|c| {
        c.set(c.get() + 1);
        c.get() == 1
    })
}

/// `catch_unwind`, after which no panic of this call is in flight any more.
fn catch<R>(call: impl FnOnce() -> R + std::panic::UnwindSafe) -> Option<R> {
    let r = std::panic::catch_unwind(call).ok();
    PANICS.with(|c| c.set(0));
    r
}

/// Run `call` as a `CDCICS` call: [`in_cdcics`] is true meanwhile, a panic is caught (None).
fn guarded<R>(call: impl FnOnce() -> R + std::panic::UnwindSafe) -> Option<R> {
    let was = IN_CDCICS.with(|c| c.replace(true));
    let r = catch(call);
    IN_CDCICS.with(|c| c.set(was));
    r
}

/// Test seams, in debug builds only (release builds have none): a bridge panic on demand, and an EXEC CICS
/// call without GnuCOBOL. Used by carddemo's tests.
#[cfg(debug_assertions)]
mod test_seam {
    use std::sync::atomic::{AtomicU8, Ordering};

    /// The next EXEC CICS command panics: 1 = an ordinary panic, 2 = one that cannot unwind.
    pub static NEXT: AtomicU8 = AtomicU8::new(0);

    /// `CARDDEMO_TEST_PANIC=unwind|nounwind`: the first command of the process panics that way.
    pub fn from_env() {
        match std::env::var("CARDDEMO_TEST_PANIC").as_deref() {
            Ok("unwind") => NEXT.store(1, Ordering::SeqCst),
            Ok("nounwind") => NEXT.store(2, Ordering::SeqCst),
            _ => {}
        }
    }

    pub fn fire() {
        match NEXT.swap(0, Ordering::SeqCst) {
            1 => panic!("test seam: EXEC CICS command panics"),
            // std's debug-build precondition check: a panic that cannot unwind, the process aborts
            2 => _ = unsafe { std::slice::from_raw_parts::<u8>(std::hint::black_box(std::ptr::null()), 1) },
            _ => {}
        }
    }
}

/// Run `f` the way the bridge runs an EXEC CICS command: [`in_cdcics`] is true meanwhile and a panic is
/// caught (None). A test seam (debug builds): tests of panic hooks without GnuCOBOL.
#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn as_cdcics_call<R>(f: impl FnOnce() -> R + std::panic::UnwindSafe) -> Option<R> {
    guarded(f)
}

/// Make the next EXEC CICS command of a COBOL program panic, as a bridge bug would. A test seam (debug
/// builds): the ASRA path through the real entry point.
#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn panic_in_next_command() {
    test_seam::NEXT.store(1, Ordering::SeqCst);
}

/// The `CDCICS` entry point called by the translated COBOL programs. No panic may unwind into libcob
/// (that aborts the process with the terminal in raw mode), so everything runs inside [`guarded`]: a
/// panicked command ends the task with ASRA (EIBRESP/EIBRESP2 set to 0 as for any abend, an active
/// HANDLE ABEND LABEL takes it); if that handling panics too, the task ends with ASRA directly.
extern "C" fn cdcics() -> c_int {
    guarded(|| catch(cdcics_call).unwrap_or_else(command_panicked)).unwrap_or_else(|| {
        let ctx = CTX.with(|c| c.get());
        if !ctx.is_null() {
            unsafe { (*ctx).transfer = Some(Transfer::Abend("ASRA".into())) };
        }
        RC_TRANSFER
    })
}

/// A command panicked (a bridge bug): the abend CICS gives a program check, ASRA.
fn command_panicked() -> c_int {
    let ctx = CTX.with(|c| c.get());
    if ctx.is_null() {
        return RC_TRANSFER;
    }
    let ctx = unsafe { &mut *ctx };
    set_resp(unsafe { param_bytes(1) }, unsafe { &mut *ctx.task }, 0, 0);
    abend(ctx, "ASRA".into())
}

/// EIBRESP/EIBRESP2 in the program's EIB, and the task's copy.
fn set_resp(eib: &mut [u8], t: &mut Task, resp: i32, resp2: i32) {
    if eib.len() >= EIB_RESP2 + 4 {
        eib[EIB_RESP..EIB_RESP + 4].copy_from_slice(&resp.to_be_bytes());
        eib[EIB_RESP2..EIB_RESP2 + 4].copy_from_slice(&resp2.to_be_bytes());
    }
    t.eibresp = resp;
}

fn cdcics_call() -> c_int {
    #[cfg(debug_assertions)]
    test_seam::fire();
    let ctx = CTX.with(|c| c.get());
    if ctx.is_null() {
        return RC_TRANSFER;
    }
    let ctx = unsafe { &mut *ctx };
    let t = unsafe { &mut *ctx.task };
    let eib = unsafe { param_bytes(1) };
    let pieces: usize = unsafe { cobol::text::s(param_bytes(2)) }.trim().get(1..).and_then(|n| n.parse().ok()).unwrap_or(1);
    let desc: String = (0..pieces).map(|i| unsafe { cobol::text::s(param_bytes(3 + i as c_int)) }).collect();
    let cmd = Cmd::parse(&desc, 3 + pieces as c_int);
    let out = execute(ctx, t, &cmd);
    let (resp, resp2) = match &out {
        Out::Cond(r, r2) => (*r, *r2),
        _ => (0, 0),
    };
    set_resp(eib, t, resp, resp2);
    match out {
        Out::Ok => 0,
        Out::Cond(..) if cmd.has("RESP") || cmd.has("NOHANDLE") => 0,
        Out::Cond(r, _) => {
            if let Some(&label) = ctx.handle.get(cond_name(r)) {
                return label;
            }
            abend(ctx, cond_abend(r))
        }
        Out::Transfer(Transfer::Abend(code)) => abend(ctx, code),
        Out::Transfer(tr) => {
            ctx.transfer = Some(tr);
            RC_TRANSFER
        }
    }
}

/// An abend: taken by an active HANDLE ABEND LABEL (which is then cancelled), otherwise ends the task.
fn abend(ctx: &mut Ctx, code: String) -> c_int {
    if let Some(label) = ctx.abend_label.take() {
        return label;
    }
    ctx.transfer = Some(Transfer::Abend(code));
    RC_TRANSFER
}

fn execute(ctx: &mut Ctx, t: &mut Task, c: &Cmd) -> Out {
    let verb = c.words.first().map(String::as_str).unwrap_or("");
    match verb {
        "SEND" if c.arg("MAP").is_some() => send_map(t, c),
        "SEND" => {
            t.send_text(c.from());
            Out::Ok
        }
        "RECEIVE" => receive_map(t, c),
        "RETURN" => {
            let transid = c.arg("TRANSID").map(|_| c.text("TRANSID"));
            let commarea = c.arg("COMMAREA").map(|_| {
                let b = c.bytes("COMMAREA");
                let n = c.num("LENGTH").map_or(b.len(), |l| (l.max(0) as usize).min(b.len()));
                b[..n].to_vec()
            });
            Out::Transfer(Transfer::Return { transid, commarea })
        }
        "XCTL" => {
            let program = c.text("PROGRAM");
            if !t.program_exists(&program) {
                return Out::Cond(PGMIDERR, 3);
            }
            let commarea = c.arg("COMMAREA").map(|_| {
                let b = c.bytes("COMMAREA");
                let n = c.num("LENGTH").map_or(b.len(), |l| (l.max(0) as usize).min(b.len()));
                b[..n].to_vec()
            });
            Out::Transfer(Transfer::Xctl { program, commarea })
        }
        "ABEND" => Out::Transfer(Transfer::Abend(c.text("ABCODE"))),
        "HANDLE" => {
            if c.has("ABEND") {
                ctx.abend_label = if c.has("CANCEL") { None } else { c.labels.get("LABEL").copied() };
            } else {
                for (k, v) in &c.labels {
                    ctx.handle.insert(k.clone(), *v);
                }
            }
            Out::Ok
        }
        "READ" => {
            let (file, key) = (c.file(), c.key());
            let r = if c.has("UPDATE") { t.read(&file, &key, true).map(|r| (r, false)) } else { t.read_dup(&file, &key) };
            match r {
                Ok((rec, dup)) => {
                    copy_into(c, &rec);
                    if dup {
                        Out::Cond(Resp::DupKey.code(), 0)
                    } else {
                        Out::Ok
                    }
                }
                Err(e) => cond(e),
            }
        }
        "WRITE" => t.write(&c.file(), c.from()).map_or_else(cond, |_| Out::Ok),
        "REWRITE" => t.rewrite(&c.file(), c.from()).map_or_else(cond, |_| Out::Ok),
        "DELETE" => {
            let key = c.arg("RIDFLD").map(|_| c.key());
            t.delete(&c.file(), key.as_deref()).map_or_else(cond, |_| Out::Ok)
        }
        "STARTBR" => {
            let file = c.file();
            match t.startbr(&file, &c.key(), !c.has("EQUAL")) {
                Ok(b) => {
                    ctx.browses.insert(file, b);
                    Out::Ok
                }
                Err(e) => cond(e),
            }
        }
        "READNEXT" | "READPREV" => {
            let file = c.file();
            let Some(b) = ctx.browses.get_mut(&file) else { return Out::Cond(INVREQ, 35) };
            let r = if verb == "READNEXT" { t.readnext(b) } else { t.readprev(b) };
            match r {
                Ok(rec) => {
                    copy_into(c, &rec);
                    // RIDFLD receives the key of the record read
                    if let Ok(p) = t.vsam.path(&file) {
                        let (off, len) = p.aix.map_or((p.ds.key_off, p.ds.key_len), |a| (a.off, a.len));
                        let rid = c.bytes("RIDFLD");
                        let n = len.min(rid.len());
                        rid[..n].copy_from_slice(&rec[off..off + n]);
                    }
                    Out::Ok
                }
                Err(e) => cond(e),
            }
        }
        "ENDBR" => match ctx.browses.remove(&c.file()) {
            Some(_) => Out::Ok,
            None => Out::Cond(INVREQ, 35),
        },
        "SYNCPOINT" => {
            if c.has("ROLLBACK") {
                t.syncpoint_rollback();
            } else {
                t.syncpoint();
            }
            Out::Ok
        }
        "WRITEQ" => {
            let q = c.text("QUEUE");
            t.writeq_td(&q, c.from());
            Out::Ok
        }
        "INQUIRE" => {
            if t.program_exists(&c.text("PROGRAM")) {
                Out::Ok
            } else {
                Out::Cond(PGMIDERR, 3)
            }
        }
        "ASKTIME" => {
            let now = cobol::clock::now();
            let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
            c.put_num("ABSTIME", (now - base).num_milliseconds());
            Out::Ok
        }
        "FORMATTIME" => {
            let ms = c.num("ABSTIME").unwrap_or(0);
            let base = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
            let at = base + chrono::Duration::milliseconds(ms);
            if c.arg("YYYYMMDD").is_some() {
                let sep = if c.arg("DATESEP").is_some() { c.text("DATESEP") } else { String::new() };
                cobol::text::mv(c.bytes("YYYYMMDD"), at.format(&format!("%Y{sep}%m{sep}%d")).to_string().as_bytes());
            }
            if c.arg("TIME").is_some() {
                let sep = if c.arg("TIMESEP").is_some() { c.text("TIMESEP") } else { String::new() };
                cobol::text::mv(c.bytes("TIME"), at.format(&format!("%H{sep}%M{sep}%S")).to_string().as_bytes());
            }
            Out::Ok
        }
        "ASSIGN" => {
            if c.arg("APPLID").is_some() {
                cobol::text::mv(c.bytes("APPLID"), t.applid.as_bytes());
            }
            if c.arg("SYSID").is_some() {
                cobol::text::mv(c.bytes("SYSID"), t.sysid.as_bytes());
            }
            Out::Ok
        }
        other => {
            t.log.push(format!("cobridge: unsupported EXEC CICS {other} ({:?})", c.words));
            Out::Cond(INVREQ, 0)
        }
    }
}

fn copy_into(c: &Cmd, rec: &[u8]) {
    let into = c.bytes("INTO");
    let n = into.len().min(rec.len());
    into[..n].copy_from_slice(&rec[..n]);
    c.put_num("LENGTH", rec.len() as i64);
}

// ---------------------------------------------------------------------------- symbolic maps

/// Offsets of the named fields in the BMS symbolic map (12-byte prefix; per field: length S9(4) COMP,
/// flag/attribute byte, colour, PS, highlight, validation, then the data). Returns (field index,
/// offset) pairs and the total length.
pub fn layout(def: &MapDef) -> (Vec<(usize, usize)>, usize) {
    let mut off = 12;
    let mut v = Vec::new();
    for (i, f) in def.fields.iter().enumerate() {
        if f.name.is_some() {
            v.push((i, off));
            off += 7 + f.len;
        }
    }
    (v, off)
}

fn attr_byte(b: u8) -> Option<AttrByte> {
    match b {
        0x61 => Some(AttrByte::ProtFset),
        0x60 => Some(AttrByte::Prot),
        0xC1 => Some(AttrByte::UnprotFset),
        0x4C => Some(AttrByte::Dark),
        0xC8 => Some(AttrByte::Bright),
        0xF0 | 0xF1 => Some(AttrByte::Askip),
        0xF8 => Some(AttrByte::AskipBright),
        0x40 => Some(AttrByte::Unprot),
        _ => None,
    }
}

fn color_byte(b: u8) -> Option<Color> {
    match b {
        0xF1 => Some(Color::Blue),
        0xF2 => Some(Color::Red),
        0xF3 => Some(Color::Pink),
        0xF4 => Some(Color::Green),
        0xF5 => Some(Color::Turquoise),
        0xF6 => Some(Color::Yellow),
        0xF7 => Some(Color::Neutral),
        // X'00' (DFHDFCOL or untouched) and anything else: the map's default colour
        _ => None,
    }
}

fn send_map(t: &mut Task, c: &Cmd) -> Out {
    let mut map = t.map(&c.text("MAP"));
    let buf = c.bytes("FROM");
    let (lay, total) = layout(&map.def);
    if buf.len() < total {
        t.log.push(format!("cobridge: symbolic map {} is {} bytes, expected {total}", map.def.name, buf.len()));
        return Out::Cond(INVREQ, 0);
    }
    for (i, off) in lay {
        let len = map.def.fields[i].len;
        let f = &mut map.fields[i];
        f.data.copy_from_slice(&buf[off + 7..off + 7 + len]);
        if c.has("CURSOR") && i16::from_be_bytes([buf[off], buf[off + 1]]) == -1 {
            f.len = -1;
        }
        f.attr = attr_byte(buf[off + 2]);
        f.color = color_byte(buf[off + 3]);
    }
    t.send_map(&map, c.has("ERASE"));
    Out::Ok
}

fn receive_map(t: &mut Task, c: &Cmd) -> Out {
    let map = t.receive_map(&c.text("MAP"));
    let buf = c.bytes("INTO");
    let (lay, total) = layout(&map.def);
    if buf.len() < total {
        t.log.push(format!("cobridge: symbolic map {} is {} bytes, expected {total}", map.def.name, buf.len()));
        return Out::Cond(INVREQ, 0);
    }
    for (i, off) in lay {
        let f = &map.fields[i];
        buf[off..off + 2].copy_from_slice(&f.len.max(0).to_be_bytes());
        buf[off + 2] = 0;
        buf[off + 7..off + 7 + f.data.len()].copy_from_slice(&f.data);
    }
    Out::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_panic_of_a_command_is_caught() {
        assert!(!note_panic(), "outside a command");
        let seen = guarded(|| (note_panic(), note_panic()));
        assert_eq!(seen, Some((true, false)), "a second panic in flight means an abort");
        // a caught panic is over: the next one (here in a new command) is caught again
        let panicked = guarded(|| -> bool { std::panic::resume_unwind(Box::new(())) });
        assert_eq!(panicked, None);
        assert_eq!(guarded(note_panic), Some(true));
        assert!(!in_cdcics());
    }
}
