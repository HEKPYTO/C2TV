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

static uint32_t rotl32(uint32_t x, unsigned r) {
    return (x << r) | (x >> (32u - r));
}

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    uint32_t h = 0;
    uint32_t tail = n & ~3u;
    uint32_t rem = n & 3u;
    uint32_t i, k;
    *status = 0;
    *ret = 0;
    *outn = 0;
    (void)out;
    for (i = 0; i < tail; i += 4) {
        uint32_t v = rd32(in + i);
        v *= 0xcc9e2d51u;
        v = rotl32(v, 15);
        v *= 0x1b873593u;
        h ^= v;
        h = rotl32(h, 13);
        h = h * 5u + 0xe6546b64u;
    }
    k = 0;
    if (rem >= 3) k ^= (uint32_t)in[tail + 2] << 16;
    if (rem >= 2) k ^= (uint32_t)in[tail + 1] << 8;
    if (rem >= 1) {
        k ^= (uint32_t)in[tail];
        k *= 0xcc9e2d51u;
        k = rotl32(k, 15);
        k *= 0x1b873593u;
        h ^= k;
    }
    h ^= n;
    h ^= h >> 16;
    h *= 0x85ebca6bu;
    h ^= h >> 13;
    h *= 0xc2b2ae35u;
    h ^= h >> 16;
    *ret = (uint64_t)h;
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
