#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define CAP 4096

static int read_all(uint8_t *dst, size_t n) {
    size_t got = 0;
    while (got < n) {
        size_t r = fread(dst + got, 1, n - got, stdin);
        if (r == 0) return 0;
        got += r;
    }
    return 1;
}

static uint32_t rd32(const uint8_t *p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static uint64_t rd64(const uint8_t *p) {
    return (uint64_t)rd32(p) | ((uint64_t)rd32(p + 4) << 32);
}

static void wr32(uint8_t *p, uint32_t v) {
    p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8);
    p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24);
}

static void wr64(uint8_t *p, uint64_t v) {
    wr32(p, (uint32_t)v); wr32(p + 4, (uint32_t)(v >> 32));
}

static int count_scalars(const uint8_t *p, uint32_t n, uint32_t *count) {
    uint32_t i = 0;
    uint32_t c = 0;
    while (i < n) {
        uint8_t b = p[i];
        uint32_t need;
        uint8_t lo = 0x80;
        uint8_t hi = 0xbf;
        uint32_t k;
        if (b < 0x80) {
            i++;
            c++;
            continue;
        }
        if (b >= 0xc2 && b <= 0xdf) need = 1;
        else if (b == 0xe0) { need = 2; lo = 0xa0; }
        else if (b >= 0xe1 && b <= 0xec) need = 2;
        else if (b == 0xed) { need = 2; hi = 0x9f; }
        else if (b >= 0xee && b <= 0xef) need = 2;
        else if (b == 0xf0) { need = 3; lo = 0x90; }
        else if (b >= 0xf1 && b <= 0xf3) need = 3;
        else if (b == 0xf4) { need = 3; hi = 0x8f; }
        else return 0;
        if (i + need >= n) return 0;
        if (p[i + 1] < lo || p[i + 1] > hi) return 0;
        for (k = 2; k <= need; k++) {
            if ((p[i + k] & 0xc0) != 0x80) return 0;
        }
        i += need + 1;
        c++;
    }
    *count = c;
    return 1;
}

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    uint32_t count = 0;
    *status = 0;
    *ret = 0;
    *outn = 0;
    (void)out;
    if (!count_scalars(in, n, &count)) {
        *status = 1;
        return;
    }
    *ret = (uint64_t)count;
}

int main(void) {
    uint8_t hdr[4];
    uint8_t in[CAP];
    uint8_t out[CAP];
    uint8_t resp[20 + CAP];
    uint32_t n;
    uint64_t status = 0;
    uint64_t ret = 0;
    uint32_t outn = 0;
    if (!read_all(hdr, 4)) return 2;
    n = rd32(hdr);
    if (n > CAP) return 2;
    if (!read_all(in, n)) return 2;
    compute(in, n, &status, &ret, out, &outn);
    if (outn > CAP) return 2;
    wr64(resp, status);
    wr64(resp + 8, ret);
    wr32(resp + 16, outn);
    if (outn != 0) memcpy(resp + 20, out, outn);
    if (fwrite(resp, 1, 20 + outn, stdout) != (size_t)(20 + outn)) return 2;
    return 0;
}
