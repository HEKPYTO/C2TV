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

static int hexval(uint8_t c) {
    if (c >= (uint8_t)'0' && c <= (uint8_t)'9') return (int)(c - (uint8_t)'0');
    if (c >= (uint8_t)'a' && c <= (uint8_t)'f') return (int)(c - (uint8_t)'a') + 10;
    if (c >= (uint8_t)'A' && c <= (uint8_t)'F') return (int)(c - (uint8_t)'A') + 10;
    return -1;
}

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    static const uint8_t digits[] = "0123456789abcdef";
    uint32_t i;
    int hi, lo;
    *status = 0;
    *ret = 0;
    *outn = 0;
    if (n < 1) { *status = 1; return; }
    if (in[0] == 0) {
        for (i = 1; i < n; i++) {
            out[2u * (i - 1)] = digits[in[i] >> 4];
            out[2u * (i - 1) + 1u] = digits[in[i] & 15u];
        }
        *outn = 2u * (n - 1);
        *ret = (uint64_t)*outn;
    } else if (in[0] == 1) {
        if (((n - 1u) & 1u) != 0) { *status = 1; return; }
        for (i = 1; i < n; i += 2) {
            hi = hexval(in[i]);
            lo = hexval(in[i + 1]);
            if (hi < 0 || lo < 0) { *status = 1; return; }
            out[(i - 1u) / 2u] = (uint8_t)((hi << 4) | lo);
        }
        *outn = (n - 1u) / 2u;
        *ret = (uint64_t)*outn;
    } else {
        *status = 1;
    }
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
