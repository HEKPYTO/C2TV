#include <stdint.h>
#include <stdio.h>

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

static void wr32(uint8_t *p, uint32_t v) {
    p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8);
    p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24);
}

static void wr64(uint8_t *p, uint64_t v) {
    wr32(p, (uint32_t)v); wr32(p + 4, (uint32_t)(v >> 32));
}

static int32_t as_i32(uint32_t v) {
    if (v <= 0x7fffffffu) return (int32_t)v;
    return (int32_t)((int64_t)v - 4294967296LL);
}

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    int64_t a;
    int64_t b;
    int64_t sum;
    int32_t r;
    *status = 0;
    *ret = 0;
    *outn = 0;
    (void)out;
    if (n != 8) {
        *status = 1;
        return;
    }
    a = (int64_t)as_i32(rd32(in));
    b = (int64_t)as_i32(rd32(in + 4));
    sum = a + b;
    if (sum > 2147483647LL) {
        r = 2147483647;
    } else if (sum < -2147483648LL) {
        r = (-2147483647 - 1);
    } else {
        r = (int32_t)sum;
    }
    *ret = (uint64_t)(int64_t)r;
}

int main(void) {
    uint8_t hdr[4];
    uint8_t in[CAP];
    uint8_t resp[20];
    uint32_t n;
    uint64_t status = 0;
    uint64_t ret = 0;
    uint32_t outn = 0;
    if (!read_all(hdr, 4)) return 2;
    n = rd32(hdr);
    if (n > CAP) return 2;
    if (!read_all(in, n)) return 2;
    compute(in, n, &status, &ret, NULL, &outn);
    if (outn != 0) return 2;
    wr64(resp, status);
    wr64(resp + 8, ret);
    wr32(resp + 16, outn);
    if (fwrite(resp, 1, sizeof resp, stdout) != sizeof resp) return 2;
    return 0;
}
