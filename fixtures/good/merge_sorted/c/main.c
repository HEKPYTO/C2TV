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

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    uint32_t na, nb, off, i, j, k;
    uint32_t a[8], b[8];
    *status = 0;
    *ret = 0;
    *outn = 0;
    if (n < 4) { *status = 1; return; }
    na = rd32(in);
    off = 4;
    if (na > 8 || (uint64_t)off + (uint64_t)na * 4u > n) { *status = 1; return; }
    for (i = 0; i < na; i++) {
        a[i] = rd32(in + off);
        off += 4;
        if (i > 0 && a[i] < a[i - 1]) { *status = 1; return; }
    }
    if ((uint64_t)off + 4u > n) { *status = 1; return; }
    nb = rd32(in + off);
    off += 4;
    if (nb > 8 || (uint64_t)off + (uint64_t)nb * 4u != n) { *status = 1; return; }
    for (i = 0; i < nb; i++) {
        b[i] = rd32(in + off);
        off += 4;
        if (i > 0 && b[i] < b[i - 1]) { *status = 1; return; }
    }
    i = 0;
    j = 0;
    k = 0;
    while (i < na && j < nb) {
        if (a[i] <= b[j]) { wr32(out + 4u * k, a[i]); i++; }
        else { wr32(out + 4u * k, b[j]); j++; }
        k++;
    }
    while (i < na) { wr32(out + 4u * k, a[i]); i++; k++; }
    while (j < nb) { wr32(out + 4u * k, b[j]); j++; k++; }
    *outn = 4u * k;
    *ret = (uint64_t)na + (uint64_t)nb;
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
