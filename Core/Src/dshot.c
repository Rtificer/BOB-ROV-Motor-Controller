#include "dshot.h"
#include "stm32f405xx.h"
#include "utils.h"
#include <stdbool.h>
#include <stdint.h>

#define ERPM_START_MARGIN_US 5

#define TIM1_MODER_MASK                                                                            \
  ~(GPIO_MODER_MODE8 | GPIO_MODER_MODE9 | GPIO_MODER_MODE10 | GPIO_MODER_MODE11)
#define TIM8_MODER_MASK ~(GPIO_MODER_MODE6 | GPIO_MODER_MODE7 | GPIO_MODER_MODE8 | GPIO_MODER_MODE9)

#define DSHOT_TELEMETRY_INVALID (0xffff)
#define DSHOT_TELEMENTRY_NOEDGE (0xfffe)

#define MIN_VALID_IDR_SAMPLES ((DSHOT_INPUT_BIT_COUNT - 2) * DSHOT_OVERSAMPLE_RATE)
#define MAX_VALID_IDR_SAMPLES ((DSHOT_INPUT_BIT_COUNT + 2) * DSHOT_OVERSAMPLE_RATE)

// Period at which to check preamble length
#define MARGIN_CHECK_INTERVAL_US 500000

// Target 5 us of input data ahead of leading edge
#define ERPM_MARGIN_CHECK_INTERVAL_US 5

#define ERPM_MARGIN_CHECK_INTERVAL_CYCLES                                                          \
  (ERPM_MARGIN_CHECK_INTERVAL_US * (SYS_CLK_FREQ / 1000000U))
#define ERPM_START_MARGIN_CYCLES (ERPM_START_MARGIN_US * (SYS_CLK_FREQ / 1000000U))

volatile uint32_t cmd_ccr_tim1_buf[2][16][4];
volatile uint32_t cmd_ccr_tim8_buf[2][16][4];
volatile uint16_t tim1_idr_buf[2][DSHOT_INPUT_BUF_LEN];
volatile uint16_t tim8_idr_buf[2][DSHOT_INPUT_BUF_LEN];
volatile bool new_tim1_idr_data = false;
volatile bool new_tim8_idr_data = false;

static inline uint8_t dshot_crc(uint16_t data)
{
  return (~(data ^ (data >> 4) ^ (data >> 8))) & 0x0F;
}

inline void
build_dshot_frames(const uint16_t words[8], uint32_t tim1_buf[16][4], uint32_t tim8_buf[16][4])
{
  for (uint8_t ch = 0; ch < 4; ++ch) {
    // Shift right by five leaves room for crc
    uint16_t tim1_data = words[ch] << 4;
    uint16_t tim8_data = words[ch + 4] << 4;

    uint8_t tim1_crc = dshot_crc(tim1_data);
    uint8_t tim8_crc = dshot_crc(tim8_data);

    // Construct frame fields and reverse bit order (to MSB)
    // Also place the CCR high period in the buffers.

    // Data bits 5-16 -> buffer positions 1-11
    for (uint8_t i = 0; i < 12; ++i) {
      tim1_buf[i][ch] = (tim1_data >> (11 - i)) & 1 ? DSHOT_BIT1_CCR : DSHOT_BIT0_CCR;
      tim8_buf[i][ch] = (tim8_data >> (11 - i)) & 1 ? DSHOT_BIT1_CCR : DSHOT_BIT0_CCR;
    }

    // CRC bits 0-4 -> buffer position 13-16
    for (uint8_t i = 0; i < 4; ++i) {
      tim1_buf[12 + i][ch] = (tim1_crc >> (3 - i)) & 1 ? DSHOT_BIT1_CCR : DSHOT_BIT0_CCR;
      tim8_buf[12 + i][ch] = (tim8_crc >> (3 - i)) & 1 ? DSHOT_BIT1_CCR : DSHOT_BIT0_CCR;
    }
  }
}

inline void setup_dshot_pins(void)
{
  // Set DShot pins to alternate function mode
  // [RM0090 8.3.7 & Figure 26, DS8626 Table 7]
  GPIOA->MODER &= TIM1_MODER_MASK;
  GPIOA->MODER |= (2 << GPIO_MODER_MODE8_Pos) | (2 << GPIO_MODER_MODE9_Pos) |
                  (2 << GPIO_MODER_MODE10_Pos) | (2 << GPIO_MODER_MODE11_Pos);

  GPIOC->MODER &= TIM8_MODER_MASK;
  GPIOC->MODER |= (2 << GPIO_MODER_MODE6_Pos) | (2 << GPIO_MODER_MODE7_Pos) |
                  (2 << GPIO_MODER_MODE8_Pos) | (2 << GPIO_MODER_MODE9_Pos);

  // Set DShot pins to correct alternate funciton
  // [RM0090 8.3.7 & Figure 26, DS8626 Table 7]
  GPIOA->AFR[1] |= (1 << GPIO_AFRH_AFSEL8_Pos) | (1 << GPIO_AFRH_AFSEL9_Pos) |
                   (1 << GPIO_AFRH_AFSEL10_Pos) | (1 << GPIO_AFRH_AFSEL11_Pos);

  GPIOC->AFR[0] |= (3 << GPIO_AFRL_AFSEL6_Pos) | (3 << GPIO_AFRL_AFSEL7_Pos);
  GPIOC->AFR[1] |= (3 << GPIO_AFRH_AFSEL8_Pos) | (3 << GPIO_AFRH_AFSEL9_Pos);

  // Set DShot pins OSPEEDR value to high [DS8626 Table 50]
  GPIOA->OSPEEDR = (2 << GPIO_OSPEEDR_OSPEED8_Pos) | (2 << GPIO_OSPEEDR_OSPEED9_Pos) |
                   (2 << GPIO_OSPEEDR_OSPEED10_Pos) | (2 << GPIO_OSPEEDR_OSPEED11_Pos);

  GPIOC->OSPEEDR |= (2 << GPIO_OSPEEDR_OSPEED6_Pos) | (2 << GPIO_OSPEEDR_OSPEED7_Pos) |
                    (2 << GPIO_OSPEEDR_OSPEED8_Pos) | (2 << GPIO_OSPEEDR_OSPEED9_Pos);

  // Set DShot pins to pull up
  GPIOA->PUPDR = (2 << GPIO_PUPDR_PUPD8_Pos) | (2 << GPIO_PUPDR_PUPD9_Pos) |
                 (2 << GPIO_PUPDR_PUPD10_Pos) | (2 << GPIO_PUPDR_PUPD11_Pos);

  GPIOC->PUPDR = (2 << GPIO_PUPDR_PUPD6_Pos) | (2 << GPIO_PUPDR_PUPD7_Pos) |
                 (2 << GPIO_PUPDR_PUPD8_Pos) | (2 << GPIO_PUPDR_PUPD9_Pos);
}

inline void setup_dshot_timer(TIM_TypeDef *timer)
{
  timer->CR1 = (0 << TIM_CR1_UDIS_Pos)   // Enable timer update event
               | TIM_CR1_URS             // Set counter overflow as only update sourcce
               | (0 << TIM_CR1_DIR_Pos)  // Set upcounter
               | (1 << TIM_CR1_ARPE_Pos) // Enable auto reload register preload
               | (0 << TIM_CR1_CKD_Pos); // Set clock division of 1
                                         // (full speed)
  timer->DIER = TIM_DIER_UDE;            // Enable update DMA request enable

  timer->CCMR1 = (0 << TIM_CCMR1_CC1S_Pos)   // Configure as output
                 | TIM_CCMR1_OC1PE           // Enable CCR preload
                 | (6 << TIM_CCMR1_OC1M_Pos) // Set PW/M Mode 1 (active
                                             // while count < CCR)
                 | (0 << TIM_CCMR1_CC2S_Pos) | TIM_CCMR1_OC2PE | (6 << TIM_CCMR1_OC2M_Pos);
  timer->CCMR2 = (0 << TIM_CCMR2_CC3S_Pos) | TIM_CCMR2_OC3PE | (6 << TIM_CCMR2_OC3M_Pos) |
                 (0 << TIM_CCMR2_CC4S_Pos) | TIM_CCMR2_OC4PE | (6 << TIM_CCMR2_OC4M_Pos);

  timer->CCER = TIM_CCER_CC1E   // Enable capture/compare channel 1
                | TIM_CCER_CC1P // Set active low polarity during output
                | TIM_CCER_CC2E | TIM_CCER_CC2P | TIM_CCER_CC3E | TIM_CCER_CC3P | TIM_CCER_CC4E |
                TIM_CCER_CC4P;

#define CCR1_Offset (0x34 / 4)
  timer->DCR = (CCR1_Offset << TIM_DCR_DBA_Pos) // set DMA base address to CCR1
                                                // Address
               | (4 << TIM_DCR_DBL_Pos);        // Set burst length for all
                                                // for CCR adresses
}

// ARPE is enabled, so changes to ARR value don't take place
// until the next overflow event
inline void set_dshot_timer_input_mode(TIM_TypeDef *timer) { timer->ARR = DSHOT_INPUT_ARR; }

// ARPE is enabled, so changes to ARR value don't take place
// until the next overflow event
inline void set_dshot_timer_output_mode(TIM_TypeDef *timer) { timer->ARR = DSHOT_OUTPUT_ARR; }

static inline void configure_dshot_output_internal(
    DMA_Stream_TypeDef *stream,
    TIM_TypeDef *timer,
    uint32_t timer_ccr_buf[16][4])
{
  reset_dma_stream(stream);

  // Set destination peripheral pointer to full transfer
  // register
  stream->PAR = (uint32_t)&timer->DMAR;
  // Set buffer pointer
  stream->M0AR = (uint32_t)timer_ccr_buf;
  // Total of 64 transfers (4 channels * 16 bits)
  stream->NDTR = 64;
  stream->FCR = DMA_SxFCR_DMDIS             // Disable direct mode
                | (3 << DMA_SxFCR_FTH_Pos); // Set full fifo Size
}

#define DSHOT_OUTPUT_DMA_CR_CONFIG(channel)                                                        \
  (DMA_SxCR_EN                         /*Enable stream*/                                           \
   | DMA_SxCR_TCIE                     /*Enable transfer complete interrupt*/                      \
   | (1 << DMA_SxCR_DIR_Pos)           /*Memory to Peripheral*/                                    \
   | DMA_SxCR_MINC                     /*Enable memory increment mode*/                            \
   | (2 << DMA_SxCR_PSIZE_Pos)         /*Set 32-bit peripheral data size*/                         \
   | (2 << DMA_SxCR_MSIZE_Pos)         /*Set 32-bit memory data size*/                             \
   | (3 << DMA_SxCR_PL_Pos)            /*Set very high priority level*/                            \
   | (1 << DMA_SxCR_PBURST_Pos)        /*Increment 4 burst (1 for each motor)*/                    \
   | (1 << DMA_SxCR_MBURST_Pos)        /*Increment 4 burst*/                                       \
   | ((channel) << DMA_SxCR_CHSEL_Pos) /*Set channel [RM0090 Table 43 & 44]*/                      \
  )

inline void configure_dshot_output_dma(
    DMA_Stream_TypeDef *stream,
    TIM_TypeDef *timer,
    uint32_t timer_ccr_buf[16][4],
    uint8_t channel)
{
  configure_dshot_output_internal(stream, timer, timer_ccr_buf);
  stream->CR = DSHOT_OUTPUT_DMA_CR_CONFIG(channel);
}

inline void switch_to_dshot_output_dma(
    DMA_Stream_TypeDef *stream,
    TIM_TypeDef *timer,
    uint32_t timer_ccr_buf[16][4],
    uint8_t channel)
{
  configure_dshot_output_internal(stream, timer, timer_ccr_buf);

  stream->CR &= DMA_SxCR_CT;
  stream->CR |= DSHOT_OUTPUT_DMA_CR_CONFIG(channel);
}

inline void configure_dshot_input_internal(
    DMA_Stream_TypeDef *stream,
    GPIO_TypeDef *gpio,
    uint16_t idr_buf[2][DSHOT_INPUT_BUF_LEN])
{
  reset_dma_stream(stream);

  // Set peripheral register to input data register for that
  // gpio bank.
  stream->PAR = (uint32_t)&gpio->IDR;
  // Set buffer pointers
  stream->M0AR = (uint32_t)idr_buf[0];
  stream->M1AR = (uint32_t)idr_buf[1];
  stream->NDTR = DSHOT_INPUT_BUF_LEN;
  stream->FCR = 0; // Enable direct mode
}

#define DMA_INPUT_DMA_CR_CONFIG(channel)                                                           \
  (DMA_SxCR_EN                         /*Enable Stream*/                                           \
   | DMA_SxCR_TCIE                     /*Enable transfer complete interrupt*/                      \
   | (0 << DMA_SxCR_DIR_Pos)           /*Peripheral to Memory*/                                    \
   | DMA_SxCR_MINC                     /*Enable memory increment mode*/                            \
   | (1 << DMA_SxCR_PSIZE_Pos)         /*Set 16-bit peripheral data size*/                         \
   | (1 << DMA_SxCR_MSIZE_Pos)         /*Set 16-bit memory data size*/                             \
   | (3 << DMA_SxCR_PL_Pos)            /*Set very high priority level*/                            \
   | DMA_SxCR_DBM                      /*Enable double buffer mode*/                               \
   | ((channel) << DMA_SxCR_CHSEL_Pos) /*Set channel [RM0090 Table 43 & 44]*/                      \
  )

inline void switch_to_dshot_input_dma(
    DMA_Stream_TypeDef *stream,
    GPIO_TypeDef *gpio,
    uint16_t idr_buf[2][DSHOT_INPUT_BUF_LEN],
    uint8_t channel)
{
  configure_dshot_input_internal(stream, gpio, idr_buf);
  stream->CR &= DMA_SxCR_CT;
  stream->CR |= DMA_INPUT_DMA_CR_CONFIG(channel);
}

// Fragile on confiugre_dshot_output_dma() implementation
inline bool is_in_output_mode(DMA_Stream_TypeDef *stream, TIM_TypeDef *timer)
{
  return stream->PAR == (uint32_t)&timer->DMAR;
}

static uint32_t decode_bb_value(uint32_t value)
{
  static const uint32_t iv = 0xFFFFFFFF; // invalid
  value &= 0xFFFFF;

  // clang-format off
  static const uint32_t decode[32] = {
    iv, iv, iv, iv, iv, iv, iv, iv,
    iv, 9, 10, 11, iv, 13, 14, 15,
    iv, iv, 2, 3, iv, 5, 6, 7,
    iv, 0, 8, 1, iv, 4, 12, iv
  };
  // clang-format on

  uint32_t decoded_value = decode[value & 0x1f];
  decoded_value |= decode[(value >> 5) & 0x1f] << 4;
  decoded_value |= decode[(value >> 10) & 0x1f] << 8;
  decoded_value |= decode[(value >> 15) & 0x1f] << 12;

  uint32_t csum = decoded_value;
  // xor is communicative so if N=nibble
  // first xor does [N3 ^ N1] [N2 ^ N0]
  // and second xor does (N2 ^ N0) ^ (N3 ^ N1)
  // we don't care about the garbage data accumulating in higher bits since we mask anyays
  // csum/N0 is defined as ~(N1 ^ N2 ^ N3) & 0xF so we effectively do
  // (N1 ^ N2 ^ N3) ^ ~(N1 ^ N2 ^ N3), which is = 1, because ~x ^ x = 1.
  csum = csum ^ (csum >> 8); // xor bytes
  csum = csum ^ (csum >> 4); // xor nibbles

  if ((csum & 0xF) != 0xF || decoded_value > 0xFFFF) {
    value = DSHOT_TELEMETRY_INVALID;
  } else {
    value = decoded_value >> 4;
  }

  return value;
}

uint32_t decode_erpm_idr(uint16_t idr_buf[DSHOT_INPUT_BUF_LEN], uint8_t bit)
{
  static uint8_t preamble_skip = 0;

  uint32_t now = DWT->CYCCNT;
  uint32_t value = 0;

  bitBandWord_t *p = (bitBandWord_t *)BITBAND_SRAM((uint32_t)idr_buf, (uint32_t)bit);

  bitBandWord_t *beg_p = p;
  // ensure MIN_VALID_IDR_SAMPLES remain
  bitBandWord_t *end_p = p + (DSHOT_INPUT_BUF_LEN - MIN_VALID_IDR_SAMPLES);

  // Jump forward in the buffer to just before where we anticipate the first
  // zero
  p += preamble_skip;

  // Eliminate leading high signal level by looking for first zero bit in data
  // stream. Manual loop unrolling and branch hinting to produce faster code.
  while (p < end_p) {
    if (__builtin_expect((!(p++)->value), 0) || __builtin_expect((!(p++)->value), 0) ||
        __builtin_expect((!(p++)->value), 0) || __builtin_expect((!(p++)->value), 0)) {
      break;
    }
  }

  const uint32_t start_margin = p - beg_p;

  if (p >= end_p) {
    // not returning telemetry is ok if the esc cpu is overburdened.
    // In that case no edge will be found and BB_NOEDGE indicates the condition to caller
    if (preamble_skip > 0) {
      // Increase the start margin
      preamble_skip--;
    }
    return DSHOT_TELEMENTRY_NOEDGE;
  }

  const uint16_t remaining_samples =
      MIN(DSHOT_INPUT_BUF_LEN - start_margin, (uint16_t)MAX_VALID_IDR_SAMPLES);

  bitBandWord_t *old_p = p;
  uint8_t bits = 0;
  end_p = p + remaining_samples;

  while (end_p < p) {
    do {
      // Look for next positive edge.
      // Manual loop unrolling and branch hinting to produce faster code.
      if (__builtin_expect((p++)->value, 0) || __builtin_expect((p++)->value, 0) ||
          __builtin_expect((p++)->value, 0) || __builtin_expect((p++)->value, 0)) {
        break;
      }
    } while (end_p > p);

    if (end_p > p) {
      // A level of length n gets decoded to a sequence of bits of
      // the form 1 followed by len - 1 = ((n+1) / DSHOT_OVERSAMPLE_RATE) - 1 0s to account for
      // oversampling.
      const uint8_t len = MAX((p - old_p + 1) / DSHOT_OVERSAMPLE_RATE, 1);
      bits += len;
      value <<= len;
      value |= 1 << (len - 1);
      old_p = p;

      // Look for next zero edge. Manual loop unrolling and branch hinting to produce faster code.
      do {
        if (__builtin_expect(!(p++)->value, 0) || __builtin_expect(!(p++)->value, 0) ||
            __builtin_expect(!(p++)->value, 0) || __builtin_expect(!(p++)->value, 0)) {
          break;
        }
      } while (end_p > p);

      if (end_p > p) {

        // A level of length n gets decoded to a sequence of bits of
        // the form 1 followed by len - 1 = ((n+1) / DSHOT_OVERSAMPLE_RATE) - 1 0s to account for
        // oversampling.
        const uint8_t len = MAX((p - old_p + 1) / DSHOT_OVERSAMPLE_RATE, 1);
        bits += len;
        value <<= len;
        value |= 1 << (len - 1);
        old_p = p;
      }
    }
  }

  // length of last sequence has to be inferred since the last bit with inverted dshot is high
  if (bits < 18) { return DSHOT_TELEMENTRY_NOEDGE; }

  const int8_t rem_bits = DSHOT_INPUT_BIT_COUNT - bits;
  if (rem_bits < 0) { return DSHOT_TELEMENTRY_NOEDGE; }

  // Data appears valid

  static uint32_t min_margin = UINT32_MAX;
  if (start_margin < min_margin) { min_margin = start_margin; }

  static uint32_t next_margin_check_cycles = 0;
  if (next_margin_check_cycles >= now) {
    next_margin_check_cycles += ERPM_MARGIN_CHECK_INTERVAL_CYCLES;

    // Handle a skipped check
    if (next_margin_check_cycles < now) {
      next_margin_check_cycles = now + ERPM_START_MARGIN_CYCLES;
    }

    if (min_margin > ERPM_START_MARGIN_CYCLES) {
      preamble_skip = min_margin - ERPM_START_MARGIN_CYCLES;
    } else {
      preamble_skip = 0;
    }

    min_margin = UINT32_MAX;
  }

  // The anticipated edges were observed
  if (rem_bits > 0) {
    value <<= rem_bits;
    value |= 1 << (rem_bits - 1);
  }

  return decode_bb_value(value);
}

inline void process_timer_idr_data(
    uint8_t bit_offset,
    uint16_t idr_buf[DSHOT_INPUT_BUF_LEN],
    uint16_t value_buf[4],
    bool *new_idr_data_flag)
{
  for (uint8_t i = 0; i < 4; ++i) {
    uint32_t result = decode_erpm_idr(idr_buf, i + bit_offset);
    if (__builtin_expect(result == DSHOT_TELEMETRY_INVALID, false)) {
      result = 0x0EFF;
    } else if (__builtin_expect(result == DSHOT_TELEMENTRY_NOEDGE, false)) {
      result = 0x0EFE;
    } else {
      // If result interferes with extended encoding
      if ((result & 0x300) == 0x200) {
        uint8_t data = result;
        result &= 0x300;
        result |= ((uint16_t)data << 1);
      }
    }
    value_buf[i] = (uint16_t)result;
  }
  *new_idr_data_flag = false;
}

inline void dshot_dma_tranfer_complete_interrupt_handler(
    DMA_Stream_TypeDef *stream,
    TIM_TypeDef *timer,
    GPIO_TypeDef *gpio,
    uint32_t gpio_moder_sel_mask,
    uint16_t idr_buf[2][DSHOT_INPUT_BUF_LEN],
    uint8_t channel,
    uint32_t cmd_ccr_buf[16][4],
    bool *new_data_flag)
{
  if (is_in_output_mode(stream, timer)) {
    set_dshot_timer_input_mode(timer);

    // clear overflow flag
    timer->SR &= ~TIM_SR_UIF;

    // wait for final bit to be clocked out
    while (!(timer->SR & TIM_SR_UIF))
      ;
    timer->SR &= ~TIM_SR_UIF;

    // Set all to input ISR mode
    gpio->MODER &= gpio_moder_sel_mask;

    switch_to_dshot_input_dma(stream, gpio, idr_buf, channel);
    DMA2_Stream5->CR |= DMA_SxCR_EN; // Enable DMA stream
  } else {
    set_dshot_timer_output_mode(timer);
    switch_to_dshot_output_dma(stream, timer, cmd_ccr_buf, channel);
    *new_data_flag = true;
  }
}

void DMA2_Stream5_IRQHandler(void)
{
  DMA2->HIFCR = DMA_HIFCR_CTCIF5;
  dshot_dma_tranfer_complete_interrupt_handler(
      DMA2_Stream5,
      TIM1,
      GPIOA,
      TIM1_MODER_MASK,
      tim1_idr_buf,
      6,
      cmd_ccr_tim1_buf[cmd_ccr_front],
      &new_tim1_idr_data);
}

void DMA2_Stream1_IRQHandler(void)
{
  DMA2->LIFCR = DMA_LIFCR_CTCIF1;
  dshot_dma_tranfer_complete_interrupt_handler(
      DMA2_Stream1,
      TIM8,
      GPIOC,
      TIM8_MODER_MASK,
      tim8_idr_buf,
      7,
      cmd_ccr_tim8_buf[cmd_ccr_front],
      &new_tim8_idr_data);
}
