#ifndef UTILS_H
#define UTILS_H

#include "stm32f4xx.h"
#include <stdint.h>

#define SYS_CLK_FREQ (168000000UL)

#define ROUND_DIV(a, b) (((a) + (b) / 2) / (b))

#define MIN(a, b)                                                                                  \
  __extension__({                                                                                  \
    __typeof__(a) _a = (a);                                                                        \
    __typeof__(b) _b = (b);                                                                        \
    _a < _b ? _a : _b;                                                                             \
  })

#define MAX(a, b)                                                                                  \
  __extension__({                                                                                  \
    __typeof__(a) _a = (a);                                                                        \
    __typeof__(b) _b = (b);                                                                        \
    _a > _b ? _a : _b;                                                                             \
  })

#define BITBAND_SRAM_REF 0x20000000
#define BITBAND_SRAM_BASE 0x22000000
// Convert SRAM address
#define BITBAND_SRAM(a, b) ((BITBAND_SRAM_BASE + (((a) - BITBAND_SRAM_REF) << 5) + ((b) << 2)))

typedef struct bitBandWord_s {
  uint32_t value;
  uint32_t junk[15];
} bitBandWord_t;

static inline void unpack_12bit(const uint8_t in[12], uint16_t out[8])
{
  out[0] = (in[0] << 4) | (in[1] >> 4);
  out[1] = ((in[1] & 0x0F) << 8) | in[2];
  out[2] = (in[3] << 4) | (in[4] >> 4);
  out[3] = ((in[4] & 0x0F) << 8) | in[5];
  out[4] = (in[6] << 4) | (in[7] >> 4);
  out[5] = ((in[7] & 0x0F) << 8) | in[8];
  out[6] = (in[9] << 4) | (in[10] >> 4);
  out[7] = ((in[10] & 0x0F) << 8) | in[11];
}

static inline void pack_12bit(uint16_t in[8], uint8_t out[12])
{
  out[0] = in[0] >> 4;
  out[1] = (in[0] << 4) | (in[1] >> 8);
  out[2] = in[1];
  out[3] = in[2] >> 4;
  out[4] = (in[2] << 4) | (in[3] >> 8);
  out[5] = in[3];
  out[6] = in[4] >> 4;
  out[7] = (in[4] << 4) | (in[5] >> 8);
  out[8] = in[5];
  out[9] = in[6] >> 4;
  out[10] = (in[6] << 4) | (in[7] >> 8);
  out[11] = in[7];
}

inline void reset_dma_stream(DMA_Stream_TypeDef *stream)
{
  stream->CR = 0;
  while (stream->CR & DMA_SxCR_EN)
    ;
}
#endif // !UTILS_H
