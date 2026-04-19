cmake_minimum_required(VERSION 3.20)
set(CMAKE_PROJECT_NAME motor-controller)
set(CMAKE_TOOLCHAIN_FILE ${CMAKE_SOURCE_DIR}/arm-none-eabi-toolchain.cmake)
project(motor-controller C ASM)
enable_language(C ASM)

set(CPU_FLAGS
    -mcpu=cortex-m4
    -mthumb
    -mfpu=fpv4-sp-d16
    -mfloat-abi=hard
)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(SOURCES
  ./Core/Src/main.c
  ./Core/Src/system_stm32f4xx.c
  ./startup_stm32f405xx.s
)

add_executable(motor-controller ${SOURCES})

target_compile_definitions(motor-controller PRIVATE
    STM32F405xx
    HSE_VALUE=12000000U        # Feather's 12MHz crystal
)

target_include_directories(motor-controller PRIVATE
  ./Drivers/CMSIS/Include
  ./Drivers/CMSIS/Device/ST/STM32F4xx/Include/
  ./Core/Inc/
)

target_compile_options(motor-controller PRIVATE
    ${CPU_FLAGS}
    -O2
    -Wall
    -Wextra
    -fdata-sections            # place each data item in its own section
    -ffunction-sections        # place each function in its own section
    -fno-common
)

target_link_options(motor-controller PRIVATE
    ${CPU_FLAGS}
    -T${CMAKE_SOURCE_DIR}/STM32F405XX_FLASH.ld
    -Wl,--gc-sections          # dead strip unused sections
    -Wl,-Map=motor_controller.map,--cref
    --specs=nano.specs         # newlib-nano, smaller printf etc
    --specs=nosys.specs        # no OS syscall stubs needed
)
