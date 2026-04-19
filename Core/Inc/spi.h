#ifndef SPI_H
#define SPI_H

#include <stdbool.h>
#include <stdint.h>

extern volatile uint8_t erpm_data_buf[2][12];
extern volatile uint8_t erpm_data_front;
extern volatile bool erpm_buf_write_not_in_progress;

void setup_spi_peripheral(void);
void setup_spi_pins(void);
void setup_spi_dma(void);

#endif // !SPI_H
