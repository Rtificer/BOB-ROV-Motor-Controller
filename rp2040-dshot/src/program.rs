use pio::{
    Assembler, IrqIndexMode, JmpCondition, MovDestination, MovOperation, MovSource, OutDestination,
    Program, SetDestination, WaitSource,
};

use crate::encoder::DShotSpeed;

#[derive(Debug, Clone, Copy)]
pub struct BitTimingDelays {
    pub one_high_delay: u8,
    pub zero_high_delay: u8,
    pub one_low_delay: u8,
    pub zero_low_delay: u8,
}

impl BitTimingDelays {
    pub const fn new(bit_period: u32) -> Self {
        // Protocol spec says 0.75 and 0.375, use 0.6 and 0.3 for safety margin
        let one_high = (bit_period * 3) / 5; // 60%
        let zero_high = (bit_period * 3) / 10; // 30%

        let one_low = bit_period - one_high;
        let zero_low = bit_period - zero_high;

        // 1 instruction = one cycle, overhead in cycles
        const HIGH_INSTRUCTION_OVERHEAD: u32 = 1;
        const LOW_INSTRUCTION_OVERHEAD: u32 = 5;

        // Adjust for PIO instruction overhead
        let one_high_delay = (one_high - HIGH_INSTRUCTION_OVERHEAD) as u8;
        let zero_high_delay = (zero_high - HIGH_INSTRUCTION_OVERHEAD) as u8;
        let one_low_delay = (one_low - LOW_INSTRUCTION_OVERHEAD) as u8;
        let zero_low_delay = (zero_low - LOW_INSTRUCTION_OVERHEAD) as u8;

        BitTimingDelays {
            one_high_delay,
            zero_high_delay,
            one_low_delay,
            zero_low_delay     
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FrameTimingDelays {
    pub frame_delay_count: u8,
    pub frame_delay_remainder: u8,
    pub frame_delay: u8,
}

impl FrameTimingDelays {
    pub const fn new_standard(
        bit_period: u32,
        pio_clock: u32,
        update_rate: u32
    ) -> Self {
        // Frame padding
        let frame_period = pio_clock / update_rate; // Cycles per frame
        const BITS_PER_FRAME: u32 = 16;
        let bit_transmission_time = bit_period * BITS_PER_FRAME;
        const FRAME_OVERHEAD: u32 = 1;

        let frame_delay_total = frame_period 
            - bit_transmission_time 
            - FRAME_OVERHEAD;
        
        Self::from_total_delay(frame_delay_total)
    }

    pub const fn new_bidirectional(
        dshot_bit_period: u32,
        gcr_bit_period: u32,
        pio_clock: u32,
        update_rate: u32
    ) -> Self {
        // Calculate BdDShot-specific frame timing
        let frame_period = pio_clock / update_rate; // Total cycles available per frame

        const BITS_PER_FRAME: u32 = 16;
        let transmission_time = dshot_bit_period * BITS_PER_FRAME;

        // Time spent receiving GCR telemetry at the slower rate
        // The read loop runs 21 times: once for initial positioning, then 20 data bits
        const GCR_BITS_TO_READ: u32 = 21;
        let reception_time = gcr_bit_period * GCR_BITS_TO_READ;

        // Account for the overhead of switching to input mode and back to output mode
        // This includes: set PINDIRS=0, wait for pin low, wait for pin high,
        // set X=20, the initial delay, the final push, irq, set PINDIRS=1
        const BIDIRECTIONAL_OVERHEAD: u32 = 8; // Conservative estimate for mode switching

        // Calculate remaining time that needs to be filled with delays
        let frame_delay_total = frame_period
            - transmission_time
            - reception_time
            - BIDIRECTIONAL_OVERHEAD;

        Self::from_total_delay(frame_delay_total)
    }

    const fn from_total_delay(frame_delay_total: u32) -> Self {
        // Split into loop iterations (max 32 cycles per PIO delay instruction)
        const MAX_PIO_DELAY: u32 = 32;
        let frame_delay_count = (frame_delay_total / MAX_PIO_DELAY) as u8;
        let frame_delay_remainder_raw = frame_delay_total % MAX_PIO_DELAY;
        const FRAME_SETUP_OVERHEAD: u32 = 7;
        let frame_delay_remainder = (frame_delay_remainder_raw - FRAME_SETUP_OVERHEAD) as u8;

        // For the frame_delay section
        let frame_delay = ((frame_delay_total / MAX_PIO_DELAY) % MAX_PIO_DELAY) as u8;

        FrameTimingDelays {
            frame_delay_count,
            frame_delay_remainder, 
            frame_delay
        }
    }
}

// Timing configuration calculated at compile time
#[derive(Debug, Clone, Copy)]
pub struct StandardDShotTimings {
    pub bit_timings: BitTimingDelays,
    pub frame_timings: FrameTimingDelays
}

impl StandardDShotTimings {
    pub const fn new(
        dshot_speed: DShotSpeed,
        pio_clock: u32,
        update_rate: u32,
    ) -> StandardDShotTimings {
        // Calculate timing values
        let bit_period = pio_clock / dshot_speed.bit_rate_hz(); // Cycles per bit
        let bit_timings = BitTimingDelays::new(bit_period);
        let frame_timings = FrameTimingDelays::new_standard(bit_period, pio_clock, update_rate);

        StandardDShotTimings {
            bit_timings,
            frame_timings
        }
    }
}

pub struct BdDShotTimings {
    pub bit_timings: BitTimingDelays,
    pub frame_timings: FrameTimingDelays,
    pub gcr_bit_read_delay: u8,
    pub gcr_initial_read_delay: u8,
}

impl BdDShotTimings {
    pub const fn new(dshot_speed: DShotSpeed, pio_clock: u32, update_rate: u32) -> Self {
        let dshot_bit_period = pio_clock / dshot_speed.bit_rate_hz();
        let gcr_bit_period = pio_clock / dshot_speed.gcr_bit_rate_hz();

        let bit_timings = BitTimingDelays::new(dshot_bit_period);
        
        let frame_timings = FrameTimingDelays::new_bidirectional(
            dshot_bit_period, 
            gcr_bit_period, 
            pio_clock, 
            update_rate
        );

        // 1 instruction = one cycle, overhead in cycles
        const GCR_BIT_READ_OVERHEAD: u32 = 2; // in_with_delay + jmp instruction
        let gcr_bit_read_delay = (gcr_bit_period - GCR_BIT_READ_OVERHEAD) as u8;

        const GCR_INITIAL_READ_OVERHEAD: u32 = 2; // nop_with_delay itself + set X instruction
        let gcr_initial_read_delay = ((gcr_bit_period / 2) - GCR_INITIAL_READ_OVERHEAD) as u8;

        BdDShotTimings {
            bit_timings,
            frame_timings,
            gcr_bit_read_delay,
            gcr_initial_read_delay,
        }
    }
}

pub const STANDARD_DSHOT_PROGRAM_SIZE: usize = 22;
pub fn generate_standard_dshot_program(timings: &StandardDShotTimings) -> Program<STANDARD_DSHOT_PROGRAM_SIZE> {
    let mut a = Assembler::new();

    // Labels
    let mut init = a.label();
    let mut maybe_pull = a.label();
    let mut frame_delay_loop = a.label();
    let mut blocking_pull = a.label();
    let mut start_frame = a.label();
    let mut check_bit = a.label();
    let mut start_bit = a.label();
    let mut do_one = a.label();
    let mut do_zero = a.label();

    a.bind(&mut init);
    a.jmp(JmpCondition::Always, &mut blocking_pull);

    a.bind(&mut maybe_pull);
    a.mov(MovDestination::Y, MovOperation::None, MovSource::ISR);
    a.jmp(JmpCondition::YIsZero, &mut blocking_pull);
    a.pull(false, false); // noblock
    a.nop_with_delay(timings.frame_timings.frame_delay_remainder); // Repeat mode is enabled, delay is needed to control frame rat
    a.set_with_delay(SetDestination::Y, timings.frame_timings.frame_delay_count, 0);

    a.bind(&mut frame_delay_loop);
    a.jmp_with_delay(JmpCondition::YIsZero, &mut start_frame, timings.frame_timings.frame_delay);
    a.jmp(JmpCondition::YDecNonZero, &mut frame_delay_loop);

    a.bind(&mut blocking_pull);
    a.pull(false, true); // block

    a.bind(&mut start_frame); // Store the value for re-use next time
    a.mov(MovDestination::X, MovOperation::None, MovSource::OSR);
    a.jmp(JmpCondition::XIsZero, &mut blocking_pull); // wait for non-zero value
    a.out(OutDestination::Y, 16); // discard 16 most significant bits

    a.bind(&mut check_bit);
    a.jmp(JmpCondition::OutputShiftRegisterNotEmpty, &mut start_bit);
    a.jmp(JmpCondition::Always, &mut maybe_pull);

    a.bind(&mut start_bit);
    a.out(OutDestination::Y, 1);
    a.jmp(JmpCondition::YIsZero, &mut do_zero);

    a.bind(&mut do_one);
    a.set_with_delay(SetDestination::PINS, 1, timings.bit_timings.one_high_delay);
    a.set_with_delay(SetDestination::PINS, 0, timings.bit_timings.one_low_delay);
    a.jmp(JmpCondition::Always, &mut check_bit);

    a.bind(&mut do_zero);
    a.set_with_delay(SetDestination::PINS, 1, timings.bit_timings.zero_high_delay);
    a.set_with_delay(SetDestination::PINS, 0, timings.bit_timings.zero_low_delay);
    a.jmp(JmpCondition::Always, &mut check_bit);

    a.assemble_program()
}


pub const BD_DSHOT_PROGRAM_SIZE: usize = 33;
pub fn generate_bd_dshot_program(timings: &BdDShotTimings) -> Program<BD_DSHOT_PROGRAM_SIZE> {
    let mut a = Assembler::new();

    // Labels
    let mut init = a.label();
    let mut maybe_pull = a.label();
    let mut frame_delay_loop = a.label();
    let mut blocking_pull = a.label();
    let mut start_frame = a.label();
    let mut check_bit = a.label();
    let mut start_bit = a.label();
    let mut do_one = a.label();
    let mut do_zero = a.label();
    let mut wait_for_erpm = a.label();
    let mut read_bit = a.label();
    let mut cleanup_read = a.label();

    a.bind(&mut init);
    a.jmp(JmpCondition::Always, &mut blocking_pull);

    a.bind(&mut maybe_pull);
    a.mov(MovDestination::Y, MovOperation::None, MovSource::ISR);
    a.jmp(JmpCondition::YIsZero, &mut blocking_pull);
    a.pull(false, false); // noblock
    a.nop_with_delay(timings.frame_timings.frame_delay_remainder); // Repeat mode is enabled, delay is needed to control frame rat
    a.set_with_delay(
        SetDestination::Y,
        timings.frame_timings.frame_delay_count,
        0,
    );

    a.bind(&mut frame_delay_loop);
    a.jmp_with_delay(JmpCondition::YIsZero, &mut start_frame, timings.frame_timings.frame_delay);
    a.jmp(JmpCondition::YDecNonZero, &mut frame_delay_loop);

    a.bind(&mut blocking_pull);
    a.pull(false, true); // block

    a.bind(&mut start_frame); // Store the value for re-use next time
    a.mov(MovDestination::X, MovOperation::None, MovSource::OSR);
    a.jmp(JmpCondition::XIsZero, &mut blocking_pull); // wait for non-zero value
    a.out(OutDestination::Y, 16); // discard 16 most significant bits

    a.bind(&mut check_bit);
    a.jmp(JmpCondition::OutputShiftRegisterNotEmpty, &mut start_bit);
    a.jmp(JmpCondition::Always, &mut wait_for_erpm);

    a.bind(&mut start_bit);
    a.out(OutDestination::Y, 1);
    a.jmp(JmpCondition::YIsZero, &mut do_zero);

    a.bind(&mut do_one);
    a.set_with_delay(
        SetDestination::PINS,
        1,
        timings.bit_timings.one_high_delay,
    );
    a.set_with_delay(
        SetDestination::PINS,
        0,
        timings.bit_timings.one_low_delay,
    );
    a.jmp(JmpCondition::Always, &mut check_bit);

    a.bind(&mut do_zero);
    a.set_with_delay(
        SetDestination::PINS,
        1,
        timings.bit_timings.zero_high_delay,
    );
    a.set_with_delay(
        SetDestination::PINS,
        0,
        timings.bit_timings.zero_low_delay,
    );
    a.jmp(JmpCondition::Always, &mut check_bit);

    a.bind(&mut wait_for_erpm);
    a.set(SetDestination::PINDIRS, 0);
    a.wait(0, WaitSource::PIN, 0, true);
    a.wait(1, WaitSource::PIN, 0, true);
    a.set(SetDestination::X, 20);
    a.nop_with_delay(timings.gcr_initial_read_delay);

    a.bind(&mut read_bit);
    a.in_with_delay(pio::InSource::PINS, 1, timings.gcr_bit_read_delay);
    a.jmp(JmpCondition::XDecNonZero, &mut read_bit);

    a.bind(&mut cleanup_read);
    a.push(true, true);
    a.irq(false, false, 0, IrqIndexMode::REL);
    a.set(SetDestination::PINDIRS, 1);
    a.jmp(JmpCondition::Always, &mut maybe_pull);

    a.assemble_program()
}
