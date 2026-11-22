#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define CAP 4096
#define MAX_PIXELS 800

typedef struct {
    uint8_t r, g, b, a;
} Pixel;

typedef struct {
    uint32_t width, height, count;
    Pixel pixels[MAX_PIXELS];
} Image;

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

static uint32_t be32(const uint8_t *p) {
    return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) |
           ((uint32_t)p[2] << 8) | (uint32_t)p[3];
}

static void put_be32(uint8_t *p, uint32_t v) {
    p[0] = (uint8_t)(v >> 24); p[1] = (uint8_t)(v >> 16);
    p[2] = (uint8_t)(v >> 8); p[3] = (uint8_t)v;
}

static int same(Pixel a, Pixel b) {
    return a.r == b.r && a.g == b.g && a.b == b.b && a.a == b.a;
}

static unsigned int pixel_hash(Pixel p) {
    return ((unsigned int)p.r * 3u + (unsigned int)p.g * 5u +
            (unsigned int)p.b * 7u + (unsigned int)p.a * 11u) & 63u;
}

static int chan_diff(uint8_t a, uint8_t b) {
    int d = (int)a - (int)b;
    if (d > 127) d -= 256;
    if (d < -128) d += 256;
    return d;
}

static int qoi_decode(const uint8_t *data, uint32_t n, Image *img) {
    static const uint8_t end[8] = {0, 0, 0, 0, 0, 0, 0, 1};
    Pixel index[64];
    Pixel px;
    uint32_t p = 14;
    uint32_t chunks_end;
    uint32_t i;
    unsigned int run = 0;
    uint64_t count;
    if (n < 22u || data[0] != 'q' || data[1] != 'o' || data[2] != 'i' || data[3] != 'f') return 0;
    count = (uint64_t)be32(data + 4) * (uint64_t)be32(data + 8);
    if (count == 0u || count > MAX_PIXELS) return 0;
    if (data[12] < 3u || data[12] > 4u || data[13] > 1u) return 0;
    chunks_end = n - 8u;
    if (chunks_end < 14u || memcmp(data + chunks_end, end, 8) != 0) return 0;
    memset(index, 0, sizeof(index));
    px.r = 0; px.g = 0; px.b = 0; px.a = 255;
    img->width = be32(data + 4);
    img->height = be32(data + 8);
    img->count = (uint32_t)count;
    for (i = 0; i < img->count; i++) {
        if (run > 0u) {
            run--;
        } else {
            uint8_t b1;
            if (p >= chunks_end) return 0;
            b1 = data[p++];
            if (b1 == 0xfeu) {
                if (p + 3u > chunks_end) return 0;
                px.r = data[p]; px.g = data[p + 1]; px.b = data[p + 2];
                p += 3u;
            } else if (b1 == 0xffu) {
                if (p + 4u > chunks_end) return 0;
                px.r = data[p]; px.g = data[p + 1]; px.b = data[p + 2]; px.a = data[p + 3];
                p += 4u;
            } else if ((b1 & 0xc0u) == 0x00u) {
                px = index[b1 & 0x3fu];
            } else if ((b1 & 0xc0u) == 0x40u) {
                px.r = (uint8_t)((int)px.r + (((int)(b1 >> 4) & 3) - 2));
                px.g = (uint8_t)((int)px.g + (((int)(b1 >> 2) & 3) - 2));
                px.b = (uint8_t)((int)px.b + ((int)(b1 & 3) - 2));
            } else if ((b1 & 0xc0u) == 0x80u) {
                uint8_t b2;
                int vg;
                if (p >= chunks_end) return 0;
                b2 = data[p++];
                vg = (int)(b1 & 0x3fu) - 32;
                px.r = (uint8_t)((int)px.r + vg - 8 + ((int)(b2 >> 4) & 15));
                px.g = (uint8_t)((int)px.g + vg);
                px.b = (uint8_t)((int)px.b + vg - 8 + ((int)b2 & 15));
            } else if ((b1 & 0xc0u) == 0xc0u) {
                run = (unsigned int)(b1 & 0x3fu);
            } else {
                return 0;
            }
            index[pixel_hash(px)] = px;
        }
        img->pixels[i] = px;
    }
    return 1;
}

static int put_bytes(uint8_t *out, uint32_t cap, uint32_t *n, const uint8_t *src, uint32_t len) {
    if (*n + len > cap) return 0;
    memcpy(out + *n, src, len);
    *n += len;
    return 1;
}

static int put_byte(uint8_t *out, uint32_t cap, uint32_t *n, uint8_t b) {
    return put_bytes(out, cap, n, &b, 1);
}

static int qoi_encode(const Image *img, uint8_t *out, uint32_t cap, uint32_t *outn) {
    static const uint8_t magic[4] = {'q', 'o', 'i', 'f'};
    static const uint8_t end[8] = {0, 0, 0, 0, 0, 0, 0, 1};
    Pixel index[64];
    Pixel prev;
    uint32_t n = 0;
    uint32_t i;
    unsigned int run = 0;
    memset(index, 0, sizeof(index));
    prev.r = 0; prev.g = 0; prev.b = 0; prev.a = 255;
    if (!put_bytes(out, cap, &n, magic, 4)) return 0;
    if (n + 10u > cap) return 0;
    put_be32(out + n, img->width); n += 4u;
    put_be32(out + n, img->height); n += 4u;
    out[n++] = 4;
    out[n++] = 0;
    for (i = 0; i < img->count; i++) {
        Pixel px = img->pixels[i];
        if (same(px, prev)) {
            run++;
            if (run == 62u || i + 1u == img->count) {
                if (!put_byte(out, cap, &n, (uint8_t)(0xc0u | (run - 1u)))) return 0;
                run = 0;
            }
        } else {
            unsigned int idx;
            if (run > 0u) {
                if (!put_byte(out, cap, &n, (uint8_t)(0xc0u | (run - 1u)))) return 0;
                run = 0;
            }
            idx = pixel_hash(px);
            if (same(index[idx], px)) {
                if (!put_byte(out, cap, &n, (uint8_t)(0x00u | idx))) return 0;
                index[idx] = px;
            } else {
                index[idx] = px;
                if (px.a == prev.a) {
                    int dr = chan_diff(px.r, prev.r);
                    int dg = chan_diff(px.g, prev.g);
                    int db = chan_diff(px.b, prev.b);
                    if (dr >= -2 && dr <= 1 && dg >= -2 && dg <= 1 && db >= -2 && db <= 1) {
                        uint8_t b = (uint8_t)(0x40u | ((unsigned int)(dr + 2) << 4) |
                                              ((unsigned int)(dg + 2) << 2) | (unsigned int)(db + 2));
                        if (!put_byte(out, cap, &n, b)) return 0;
                    } else {
                        int vg = dg;
                        int drdg = dr - vg;
                        int dbdg = db - vg;
                        if (vg >= -32 && vg <= 31 && drdg >= -8 && drdg <= 7 &&
                            dbdg >= -8 && dbdg <= 7) {
                            if (!put_byte(out, cap, &n, (uint8_t)(0x80u | (unsigned int)(vg + 32)))) return 0;
                            if (!put_byte(out, cap, &n, (uint8_t)(((unsigned int)(drdg + 8) << 4) |
                                                                  (unsigned int)(dbdg + 8)))) return 0;
                        } else {
                            uint8_t chunk[4] = {0xfeu, px.r, px.g, px.b};
                            if (!put_bytes(out, cap, &n, chunk, 4)) return 0;
                        }
                    }
                } else {
                    uint8_t chunk[5] = {0xffu, px.r, px.g, px.b, px.a};
                    if (!put_bytes(out, cap, &n, chunk, 5)) return 0;
                }
            }
        }
        prev = px;
    }
    if (!put_bytes(out, cap, &n, end, 8)) return 0;
    *outn = n;
    return 1;
}

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    Image img;
    uint32_t encoded = 0;
    *status = 0;
    *ret = 0;
    *outn = 0;
    (void)ret;
    if (!qoi_decode(in, n, &img) || !qoi_encode(&img, out, CAP, &encoded)) {
        *status = 1;
        return;
    }
    *outn = encoded;
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
