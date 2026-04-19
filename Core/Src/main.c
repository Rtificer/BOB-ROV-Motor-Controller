#include "dshot.h"
#include "spi.h"
#include "stm32f405xx.h"
#include "utils.h"
#include <stdbool.h>
#include <stdint.h>

// buffer is 12 bytes (12 bit data * 8 motors)
volatile uint8_t erpm_data_buf[2][12];
volatile uint8_t erpm_data_front = 0;

volatile uint32_t cmd_ccr_tim1_buf[2][16][4];
volatile uint32_t cmd_ccr_tim8_buf[2][16][4];
volatile uint8_t cmd_ccr_front;

volatile bool erpm_buf_write_not_in_progress;

inline void flash_init(void)
{
  FLASH->ACR =
      // Set the flash latency to 5 cycles to accound for
      // 168MHz clock [RM0090 3.5.1 & Table 11]
      FLASH_ACR_LATENCY_5WS
      // Enable CPU Instruction prefetch, instruction cache,
      // and data cache [RM0090 3.5.2]
      | FLASH_ACR_PRFTEN | FLASH_ACR_ICEN | FLASH_ACR_DCEN;
}

inline void main_clk_init(void)
{
  // Enable configuration clock
  RCC->APB2ENR |= RCC_APB2ENR_SYSCFGEN;
  (void)RCC->APB2ENR;

  // Set clock power settings

  // Enable power interface clock
  // Nessasary to configure voltage scaling to get the
  // correct clock frequency.
  RCC->APB1ENR |= RCC_APB1ENR_PWREN;
  (void)RCC->APB1ENR;
  // Set Clock voltage to mode 1 to enable 168MHz operation
  PWR->CR |= PWR_CR_VOS;
  (void)PWR->CR;

  // Enable HSE Clock [RM0090 6.2.1]
  RCC->CR |= RCC_CR_HSEON; // Enable External Crystal
  while (!(RCC->CR & RCC_CR_HSERDY))
    ;

  // Configure Clock Scaling for 168 MHz Clock [RM0090
  // Figure 16 & 6.2.3]
  RCC->PLLCFGR =
      // Set M Prescaler to 6, PLL gets 2 MHz (12MHz / 6)
      (6 << RCC_PLLCFGR_PLLM_Pos)
      // Set N Prescaler so frequency is now 336MHz (2MHz *
      // 168)
      | (168 << RCC_PLLCFGR_PLLN_Pos)
      // Set P Prescaler to minumum value 00=/2, so 168MHz
      // (336MHz / 2)
      | (0 << RCC_PLLCFGR_PLLP_Pos)
      // Set Q Prescaler to 4 to achieve 42MHz value.
      // Prescaler must still be set within valid range to
      // use PLL despite unused peripherals.
      | (4 << RCC_PLLCFGR_PLLQ_Pos)
      // Set the PLL source to external crystal
      | RCC_PLLCFGR_PLLSRC_HSE;

  // Enable Phased Clock Loop (for 168MHz conversion)
  // [RM0090 6.2.3]
  RCC->CR |= RCC_CR_PLLON;
  while (!(RCC->CR & RCC_CR_PLLRDY))
    ;

  RCC->CFGR =
      // Set PLL as the system clock
      RCC_CFGR_SW_PLL
      // Set the AHB prescaler to no division so AHB has the
      // maximum 168MHz
      | RCC_CFGR_HPRE_DIV1
      // Set APB1 prescaler to /4 to achieve maximum 42MHz
      // value
      | RCC_CFGR_PPRE1_DIV4
      // SET APB2 prescaler to /2 to achieve maximum 84 MHz
      // value
      | RCC_CFGR_PPRE2_DIV2;
  while ((RCC->CFGR & RCC_CFGR_SWS) != RCC_CFGR_SWS_PLL)
    ;

  // Enable Clock Security System [RM0090 6.2.7]
  RCC->CR |= RCC_CR_CSSON;
}

inline void dwt_init(void)
{
  CoreDebug->DEMCR = CoreDebug_DEMCR_TRCENA_Msk;
  DWT->CYCCNT = 0;
  DWT->CTRL |= DWT_CTRL_CYCCNTENA_Msk;
}

int main(void)
{
  flash_init();

  // Set the NVIC priority grouping to [PM0214 2.3.6, 4.4.5,
  // Table 51, Table 48] This allows unique preemption
  // levels for all the used interrupts
  NVIC_SetPriorityGrouping(0);

  main_clk_init();

  dwt_init();

  // Enable Peripheral Clocks
  RCC->AHB1ENR |=
      // Enable GPIO [DS8626 Table 9 & Figure 12,
      // RM0090 6.3.10]
      RCC_AHB1ENR_GPIODEN   // For TIM4
      | RCC_AHB1ENR_GPIOBEN // For SPI2
      | RCC_AHB1ENR_GPIOCEN // For TIM8
      | RCC_AHB1ENR_GPIOHEN // For External Oscilator
      // Enable DMA1 Controller Clock
      | RCC_AHB1ENR_DMA1EN  // For SPI2
      | RCC_AHB1ENR_DMA2EN; // For TIM1 & TIM8
  (void)RCC->AHB1ENR;

  setup_spi_peripheral();
  setup_spi_pins();
  setup_spi_dma();

  setup_dshot_timer(TIM1);
  setup_dshot_timer(TIM8);

  configure_dshot_output_dma(DMA2_Stream5, TIM1, cmd_ccr_tim1_buf[cmd_ccr_front], 6);
  configure_dshot_output_dma(DMA2_Stream1, TIM8, cmd_ccr_tim8_buf[cmd_ccr_front], 7);

  setup_dshot_pins();

  TIM1->CR1 |= TIM_CR1_CEN;
  TIM8->CR1 |= TIM_CR1_CEN;

  uint16_t value_buf[8];

  while (true) {
    if (new_tim1_idr_data) {
      uint8_t front = !((DMA2_Stream5->CR >> DMA_SxCR_CT_Pos) & 1);
      process_timer_idr_data(8, tim1_idr_buf[front], value_buf, &new_tim1_idr_data);
    }
    if (new_tim8_idr_data) {
      uint8_t front = !((DMA2_Stream1->CR >> DMA_SxCR_CT_Pos) & 1);
      process_timer_idr_data(6, tim8_idr_buf[front], &value_buf[4], &new_tim8_idr_data);
    }

    // if both done processing
    if (!new_tim1_idr_data && !new_tim8_idr_data) {
      // prevent buffer switching during write
      erpm_buf_write_not_in_progress = false;
      pack_12bit(value_buf, erpm_data_buf[erpm_data_front ^ 1]);
      erpm_buf_write_not_in_progress = true;
    }
  }
}
