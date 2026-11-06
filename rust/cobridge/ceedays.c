/* CardDemo CICS bridge: CEEDAYS (Language Environment "convert date to Lilian day number"),
   for the date formats CardDemo uses (YYYY-MM-DD, YYYYMMDD). Called by the original CSUTLDTC.
   Arguments: input date and picture as varying strings (halfword length + text), output Lilian
   day number (fullword binary), 12-byte feedback token. All binary fields are big-endian. */
#include <string.h>
#include <ctype.h>

static int get16(const unsigned char *p) { return (p[0] << 8) | p[1]; }
static void put32(unsigned char *p, long v) { p[0] = v >> 24; p[1] = v >> 16; p[2] = v >> 8; p[3] = v; }

static void feedback(unsigned char *fc, int msg) {
    memset(fc, 0, 12);
    if (!msg) return;                    /* condition token of all zeros: no error */
    fc[0] = 0; fc[1] = 3;                /* severity 3 */
    fc[2] = msg >> 8; fc[3] = msg & 0xff;
    fc[4] = 0x59;                        /* case / severity / control */
    fc[5] = 0xC3; fc[6] = 0xC5; fc[7] = 0xC5;   /* facility "CEE" in EBCDIC */
}

static int dim(int y, int m) {
    static const int d[] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
    if (m == 2 && ((y % 4 == 0 && y % 100 != 0) || y % 400 == 0)) return 29;
    return d[m - 1];
}

static long lilian(int y, int m, int d) {    /* day 1 = 15 October 1582 */
    long a = (14 - m) / 12, yy = y + 4800 - a, mm = m + 12 * a - 3;
    long jdn = d + (153 * mm + 2) / 5 + 365 * yy + yy / 4 - yy / 100 + yy / 400 - 32045;
    return jdn - 2299160;
}

static int num(const unsigned char *s, int n, int *out) {
    int v = 0;
    for (int i = 0; i < n; i++) {
        if (!isdigit(s[i])) return 0;
        v = v * 10 + (s[i] - '0');
    }
    *out = v;
    return 1;
}

int CEEDAYS(unsigned char *date, unsigned char *pic, unsigned char *lil, unsigned char *fc) {
    int dl = get16(date), pl = get16(pic);
    const unsigned char *dt = date + 2, *pt = pic + 2;
    char picture[32] = {0};
    int i, n = 0;
    for (i = 0; i < pl && n < 31; i++) if (pt[i] != ' ') picture[n++] = pt[i];
    int yo, mo, doff, need;
    if (!strcmp(picture, "YYYY-MM-DD")) { yo = 0; mo = 5; doff = 8; need = 10; }
    else if (!strcmp(picture, "YYYYMMDD")) { yo = 0; mo = 4; doff = 6; need = 8; }
    else { put32(lil, 0); feedback(fc, 2518); return 0; }          /* bad picture string */
    int blank = 1;
    for (i = 0; i < need && i < dl; i++) if (dt[i] != ' ' && dt[i] != 0) blank = 0;
    if (dl < need || blank) { put32(lil, 0); feedback(fc, 2507); return 0; }   /* insufficient data */
    int y, m, d;
    if (!num(dt + yo, 4, &y) || !num(dt + mo, 2, &m) || !num(dt + doff, 2, &d)) {
        put32(lil, 0); feedback(fc, 2520); return 0;               /* non-numeric data */
    }
    if (m < 1 || m > 12) { put32(lil, 0); feedback(fc, 2517); return 0; }        /* invalid month */
    if (y < 1582 || y > 9999) { put32(lil, 0); feedback(fc, 2513); return 0; }   /* unsupported range */
    if (d < 1 || d > dim(y, m)) { put32(lil, 0); feedback(fc, 2508); return 0; } /* bad date value */
    put32(lil, lilian(y, m, d));
    feedback(fc, 0);
    return 0;
}
