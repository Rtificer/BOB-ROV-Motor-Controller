#include "spi.h"
#include "dshot.h"
#include "stm32f405xx.h"
#include "utils.h"

typedef union {
  uint16_t words[8];
  uint8_t bytes[16];
} motor_frame_t;

static volatile motor_frame_t cmd_dshot_buf;
static volatile uint8_t cmd_data_buf[12];
volatile uint8_t erpm_data_buf[2][12];
volatile uint8_t erpm_data_front = 0;
volatile bool erpm_buf_write_not_in_progress = false;

inline void setup_spi_peripheral(void)
{
  // Enable SPI2 Clock
  RCC->APB1ENR |= RCC_APB1ENR_SPI2EN;
  (void)RCC->APB1ENR;

  SPI2->CR1 =
      // Set data capture on first capture edge
      (0 << SPI_CR1_CPHA_Pos)
      // Set idle low
      | (0 << SPI_CR1_CPOL_Pos)
      // Set slave mode
      | (0 << SPI_CR1_MSTR_Pos)
      // Set MSB first
      | (0 << SPI_CR1_LSBFIRST_Pos)
      // Enable Hardware SS
      | (0 << SPI_CR1_SSM_Pos)
      // Set full duplex
      | (0 << SPI_CR1_RXONLY_Pos);

  // Set CRC polynomial to x^8 + x^5 + x^3 + x^01
  // The x^8 term in implicit, hence 0x2F
  // This ensures detection of 1, 2, and 3 bit errors
  SPI2->CRCPR = 0x2F;
  // Enable CRC
  SPI2->CR1 |= SPI_CR1_CRCEN;

  // Enable RX and TX DMA requests when RXNE and TXE flags
  // are set
  SPI2->CR2 = SPI_CR2_RXDMAEN | SPI_CR2_TXDMAEN;

  // Enable SPI peripheral
  SPI2->CR1 |= SPI_CR1_SPE;
}

inline void setup_spi_pins(void)
{
  // Set SPI pins to alternate function mode
  // [RM0090 8.3.7 & Figure 26, DS8626 Table 7]
  GPIOB->MODER &=
      ~(GPIO_MODER_MODE12_Msk | GPIO_MODER_MODE13_Msk | GPIO_MODER_MODE14_Msk |
        GPIO_MODER_MODE15_Msk);
  GPIOB->MODER |= (2 << GPIO_MODER_MODE12_Pos) | (2 << GPIO_MODER_MODE13_Pos) |
                  (2 << GPIO_MODER_MODE14_Pos) | (2 << GPIO_MODER_MODE15_Pos);

  // Set SPI pins to correct alternate funciton
  // [RM0090 8.3.7 & Figure 26, DS8626 Table 7]
  GPIOB->AFR[1] &=
      ~(GPIO_MODER_MODE12_Msk | GPIO_AFRH_AFSEL13_Msk | GPIO_AFRH_AFSEL14_Msk |
        GPIO_AFRH_AFSEL15_Msk);
  GPIOB->AFR[1] |= (5 << GPIO_AFRH_AFSEL12_Pos) | (5 << GPIO_AFRH_AFSEL13_Pos) |
                   (5 << GPIO_AFRH_AFSEL14_Pos) | (5 << GPIO_AFRH_AFSEL15_Pos);

  // Set SPI pins OSPEEDR value to medium [DS8626 Table 50]
  GPIOB->OSPEEDR &=
      ~(GPIO_OSPEEDR_OSPEED12_Msk | GPIO_OSPEEDR_OSPEED13_Msk | GPIO_OSPEEDR_OSPEED14_Msk |
        GPIO_OSPEEDR_OSPEED15_Msk);
  GPIOB->OSPEEDR |= (1 << GPIO_OSPEEDR_OSPEED12_Pos) | (1 << GPIO_OSPEEDR_OSPEED13_Pos) |
                    (1 << GPIO_OSPEEDR_OSPEED14_Pos) | (1 << GPIO_OSPEEDR_OSPEED15_Pos);
}

inline void setup_spi_dma(void)
{
  // SPI Transaction completed RX [RM0090 Table 43]
  // Priority 8 because it shouldn't interrupt timing-critical DShot interrupts
  NVIC_SetPriority(DMA1_Stream3_IRQn, 8);
  // Tell CPU to react to the interrupt flag
  NVIC_EnableIRQ(DMA1_Stream3_IRQn);

  reset_dma_stream(DMA1_Stream3);
  // Set source peripheral pointer
  DMA1_Stream3->PAR = (uint32_t)&SPI2->DR;
  // Set buffer pointer
  DMA1_Stream3->M0AR = (uint32_t)cmd_data_buf;
  // Transfer 11 bytes
  DMA1_Stream3->NDTR = 12;
  // Enabled direct mode (no FIFO)
  DMA1_Stream3->FCR = 0;

  DMA1_Stream3->CR = DMA_SxCR_TCIE               // Enable transfer complete interrupt
                     | (0 << DMA_SxCR_DIR_Pos)   // Peripheral to Memory
                     | DMA_SxCR_MINC             // Enable memory increment mode
                     | (0 << DMA_SxCR_PSIZE_Pos) // Set 8-bit peripheral data size
                     | (0 << DMA_SxCR_MSIZE_Pos) // Set 8-bit memory data size
                     // Set priority level to medium. Doesn't actually matter since TIMs and
                     // IDR are on on DMA2
                     | (1 << DMA_SxCR_PL_Pos) |
                     (0 << DMA_SxCR_CHSEL_Pos); // Set channel 0 [RM0090 Table 43]

  reset_dma_stream(DMA1_Stream4);
  // Set source periphal pointer
  DMA1_Stream4->PAR = (uint32_t)&SPI2->DR;
  // Set buffer pointer
  DMA1_Stream4->M0AR = (uint32_t)erpm_data_buf;
  // Transfer 11 bytes
  DMA1_Stream4->NDTR = 12;
  // Enable direct mode (no FIFO)
  DMA1_Stream4->FCR = 0;

  DMA1_Stream4->CR = (1 << DMA_SxCR_DIR_Pos)     // Memory to Peripheral
                     | DMA_SxCR_MINC             // Enable memory increment mode
                     | (0 << DMA_SxCR_PSIZE_Pos) // Set 8-bit peripheral data size
                     | (0 << DMA_SxCR_MSIZE_Pos) // Set 8-bit memory data size
                     // Set priority level to medium. Doesn't actually
                     // matter since TIMs and IDR are on on DMA2
                     | (1 << DMA_SxCR_PL_Pos) |
                     (0 << DMA_SxCR_CHSEL_Pos); // Set channel 0 [RM0090 Table 43]

  // Arm both streams
  DMA1_Stream3->CR |= DMA_SxCR_EN;
  DMA1_Stream4->CR |= DMA_SxCR_EN;
}

void DMA1_Stream3_IRQHandler(void)
{
  // Clear transfer complete interrupt flag
  DMA1->LIFCR = DMA_LIFCR_CTCIF3;

  // Wait for CRC byte to transfer
  while (!(SPI2->SR & SPI_SR_RXNE))
    ;
  (void)SPI2->DR; // Flush CRC

  bool crc_err = SPI2->SR & SPI_SR_CRCERR;
  if (crc_err) {
    SPI2->SR &= ~SPI_SR_CRCERR;
  } else {
    volatile uint16_t *words = cmd_dshot_buf.words;

    // Spin wait will dominate
    // do the processing while we wait to at least kill some time
    unpack_12bit(cmd_data_buf, words);
    build_dshot_frames(
        words, cmd_ccr_tim1_buf[cmd_ccr_front ^ 1], cmd_ccr_tim8_buf[cmd_ccr_front ^ 1]);

    // switch the double buffer
    cmd_ccr_front ^= 1;

    // Wait for NSS = high
    while (SPI2->SR & SPI_SR_BSY)
      ;

    // Now we can clear CRC [RM0090 28.3.6]
    SPI2->CR1 &= ~SPI_CR1_SPE;
    SPI2->CR1 &= ~SPI_CR1_CRCEN;
    SPI2->CR1 |= SPI_CR1_CRCEN;
  }

  // Reconfigure + Re-enable the DMA
  DMA1_Stream3->NDTR = 12;
  DMA1_Stream4->NDTR = 12;

  // update address if new data
  if (erpm_buf_write_not_in_progress) {
    erpm_data_front ^= 1;
    DMA1_Stream4->M0AR = (uint32_t)erpm_data_buf[erpm_data_front];
  }

  DMA1_Stream3->CR |= DMA_SxCR_EN;
  DMA1_Stream4->CR |= DMA_SxCR_EN;

  // Re-enable the SPI peripheral (does nothing if crc_err =
  // true)
  SPI2->CR1 |= SPI_CR1_SPE;
}
