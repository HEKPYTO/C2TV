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

static const char alpha[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

static uint32_t b64_encode(const uint8_t *p, uint32_t n, uint8_t *out) {
    uint32_t i = 0;
    uint32_t k = 0;
    while (i + 3 <= n) {
        uint32_t bits = ((uint32_t)p[i] << 16) |
                        ((uint32_t)p[i + 1] << 8) | (uint32_t)p[i + 2];
        out[k++] = (uint8_t)alpha[(bits >> 18) & 63u];
        out[k++] = (uint8_t)alpha[(bits >> 12) & 63u];
        out[k++] = (uint8_t)alpha[(bits >> 6) & 63u];
        out[k++] = (uint8_t)alpha[bits & 63u];
        i += 3;
    }
    if (n - i == 1) {
        uint32_t bits = (uint32_t)p[i] << 16;
        out[k++] = (uint8_t)alpha[(bits >> 18) & 63u];
        out[k++] = (uint8_t)alpha[(bits >> 12) & 63u];
        out[k++] = (uint8_t)'=';
        out[k++] = (uint8_t)'=';
    } else if (n - i == 2) {
        uint32_t bits = ((uint32_t)p[i] << 16) | ((uint32_t)p[i + 1] << 8);
        out[k++] = (uint8_t)alpha[(bits >> 18) & 63u];
        out[k++] = (uint8_t)alpha[(bits >> 12) & 63u];
        out[k++] = (uint8_t)alpha[(bits >> 6) & 63u];
        out[k++] = (uint8_t)'=';
    }
    return k;
}

static int b64val(uint8_t c, uint8_t *v) {
    if (c >= 'A' && c <= 'Z') { *v = (uint8_t)(c - 'A'); return 1; }
    if (c >= 'a' && c <= 'z') { *v = (uint8_t)(c - 'a' + 26); return 1; }
    if (c >= '0' && c <= '9') { *v = (uint8_t)(c - '0' + 52); return 1; }
    if (c == '+') { *v = 62; return 1; }
    if (c == '/') { *v = 63; return 1; }
    return 0;
}

static int b64_decode(const uint8_t *p, uint32_t n, uint8_t *out, uint32_t *outn) {
    uint32_t i;
    if (n % 4u != 0) return 0;
    *outn = 0;
    for (i = 0; i < n; i += 4) {
        uint8_t v[4] = {0, 0, 0, 0};
        uint32_t pad = 0;
        uint32_t bits;
        int seen_pad = 0;
        uint32_t k;
        for (k = 0; k < 4; k++) {
            uint8_t c = p[i + k];
            if (seen_pad) {
                if (c != '=') return 0;
                pad++;
            } else if (c == '=') {
                seen_pad = 1;
                pad = 1;
            } else if (!b64val(c, &v[k])) {
                return 0;
            }
        }
        if (pad > 2) return 0;
        if (pad == 2 && (v[1] & 0x0fu) != 0) return 0;
        if (pad == 1 && (v[2] & 0x03u) != 0) return 0;
        bits = ((uint32_t)v[0] << 18) | ((uint32_t)v[1] << 12) |
               ((uint32_t)v[2] << 6) | (uint32_t)v[3];
        out[(*outn)++] = (uint8_t)(bits >> 16);
        if (pad < 2) out[(*outn)++] = (uint8_t)(bits >> 8);
        if (pad < 1) out[(*outn)++] = (uint8_t)bits;
    }
    return 1;
}

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    *status = 0;
    *ret = 0;
    *outn = 0;
    if (n == 0) {
        *status = 1;
        return;
    }
    if (in[0] == 0) {
        *outn = b64_encode(in + 1, n - 1, out);
        return;
    }
    if (in[0] == 1) {
        if (!b64_decode(in + 1, n - 1, out, outn)) {
            *outn = 0;
            *status = 1;
            return;
        }
        *ret = (uint64_t)*outn;
        return;
    }
    *status = 1;
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
