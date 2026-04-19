#ifndef DSHOT_H
#define DSHOT_H

#include "stm32f405xx.h"
#include "utils.h"
#include <stdbool.h>
#include <stdint.h>

#define DSHOT_OUTPUT_BITRATE (600000UL)
#define DSHOT_SWITCH_PERIOD_US (30)
#define DSHOT_INPUT_BIT_COUNT (21)
#define DSHOT_INPUT_SAFTEY_MARGIN_PERCENT (10)

#define DSHOT_INPUT_BITRATE ROUND_DIV(DSHOT_OUTPUT_BITRATE * 5, 4)

#define DSHOT_OUTPUT_BIT_LENGTH_CYCLES ROUND_DIV(SYS_CLK_FREQ, DSHOT_OUTPUT_BITRATE)
// account for 0 value of counter
#define DSHOT_OUTPUT_ARR (DSHOT_OUTPUT_BIT_LENGTH_CYCLES - 1)

// 75% period
#define DSHOT_BIT1_CCR ROUND_DIV(DSHOT_OUTPUT_BIT_LENGTH_CYCLES * 3, 4)
// 37.5% period
#define DSHOT_BIT0_CCR ROUND_DIV(DSHOT_OUTPUT_BIT_LENGTH_CYCLES * 3, 8)

#define DSHOT_OVERSAMPLE_RATE (3)
#define DSHOT_INPUT_SAMPLE_RATE (DSHOT_INPUT_BITRATE * DSHOT_OVERSAMPLE_RATE)
#define DSHOT_INPUT_SAMPLE_PERIOD_CYCLES ROUND_DIV(SYS_CLK_FREQ, DSHOT_INPUT_SAMPLE_RATE)
// acount for 0 value of counter
#define DSHOT_INPUT_ARR (DSHOT_INPUT_SAMPLE_PERIOD_CYCLES - 1)

#define DSHOT_INPUT_TIME_US ROUND_DIV(DSHOT_INPUT_BIT_COUNT * 1000000UL, DSHOT_INPUT_BITRATE)
#define DSHOT_SAMPLE_WINDOW_US                                                                     \
  ROUND_DIV(                                                                                       \
      (DSHOT_SWITCH_PERIOD_US + DSHOT_INPUT_TIME_US) * (100 + DSHOT_INPUT_SAFTEY_MARGIN_PERCENT),  \
      100)
#define DSHOT_INPUT_BUF_LEN ROUND_DIV(DSHOT_SAMPLE_WINDOW_US *DSHOT_INPUT_SAMPLE_RATE, 1000000UL)

extern volatile uint32_t cmd_ccr_tim1_buf[2][16][4];
extern volatile uint32_t cmd_ccr_tim8_buf[2][16][4];
extern volatile uint8_t cmd_ccr_front;
extern volatile uint16_t tim1_idr_buf[2][DSHOT_INPUT_BUF_LEN];
extern volatile uint16_t tim8_idr_buf[2][DSHOT_INPUT_BUF_LEN];
extern volatile bool new_tim1_idr_data;
extern volatile bool new_tim8_idr_data;

void build_dshot_frames(
    const uint16_t words[8],
    uint32_t tim1_buf[16][4],
    uint32_t tim8_buf[16][4]);
void setup_dshot_pins(void);
void setup_dshot_timer(TIM_TypeDef *timer);
void calculate_erpm_idr_parsing_constants(void);
void configure_dshot_output_dma(
    DMA_Stream_TypeDef *stream,
    TIM_TypeDef *timer,
    uint32_t timer_ccr_buf[16][4],
    uint8_t channel);
void process_timer_idr_data(
    uint8_t bit_offset,
    uint16_t idr_buf[DSHOT_INPUT_BUF_LEN],
    uint16_t value_buf[4],
    bool *new_idr_data_flag);

#endif // !DSHOT_H
