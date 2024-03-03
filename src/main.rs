#![no_std]
#![no_main]

// you can put a breakpoint on `rust_begin_unwind` to catch panics
use panic_halt as _;

#[rtic::app(device = stm32f1xx_hal::pac)]
mod app {
    use rtt_target::{rprintln, rtt_init_print};
    use stm32f1xx_hal::{
        gpio::*,
        pac,
        prelude::*,
        timer::{CounterMs, CounterUs, Event},
    };

    #[shared]
    struct Shared {
        bullet_timer: CounterUs<pac::TIM2>,
        counter: u32,
    }

    #[local]
    struct Local {
        start_button: Pin<'B', 5, Input<PullDown>>,
        stop_button: Pin<'B', 6, Input<PullDown>>,
        tick_timer: CounterMs<pac::TIM1>,
        led: Pin<'C', 13, Output>,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        rtt_init_print!();

        let mut flash = ctx.device.FLASH.constrain();
        let rcc = ctx.device.RCC.constrain();

        let clocks = rcc
            .cfgr
            .use_hse(8.MHz())
            .sysclk(64.MHz())
            .freeze(&mut flash.acr);

        // Acquire the GPIOC peripheral
        let mut gpio_b = ctx.device.GPIOB.split();

        let mut start_button = gpio_b.pb5.into_pull_down_input(&mut gpio_b.crl);
        let mut stop_button = gpio_b.pb6.into_pull_down_input(&mut gpio_b.crl);
        let mut afio = ctx.device.AFIO.constrain();

        let mut gpioc = ctx.device.GPIOC.split();
        let led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

        start_button.make_interrupt_source(&mut afio);
        stop_button.make_interrupt_source(&mut afio);

        start_button.enable_interrupt(&mut ctx.device.EXTI);
        stop_button.enable_interrupt(&mut ctx.device.EXTI);

        start_button.trigger_on_edge(&mut ctx.device.EXTI, Edge::Rising);
        stop_button.trigger_on_edge(&mut ctx.device.EXTI, Edge::Rising);

        // Configure the syst timer to trigger an update every second and enables interrupt
        let mut tick_timer = ctx.device.TIM1.counter_ms(&clocks);
        tick_timer.start(1.secs()).unwrap();
        tick_timer.listen(Event::Update);


        let bullet_timer = ctx.device.TIM2.counter_us(&clocks);

        rprintln!("Init complete");
        rprintln!("{}", clocks.sysclk());

        (
            Shared {
                bullet_timer,
                counter: 0,
            },
            Local {
                start_button,
                stop_button,
                tick_timer,
                led,
            },
            init::Monotonics(),
        )
    }

    #[idle]
    fn idle(_cx: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }

    #[task(
        binds = EXTI9_5, 
        priority = 3,local = [start_button, stop_button], 
        shared = [bullet_timer, counter]
    )]
    fn button_click(mut ctx: button_click::Context) {
        if ctx.local.start_button.check_interrupt() {
            rprintln!("Start button");
            ctx.local.start_button.clear_interrupt_pending_bit();

            ctx.shared.bullet_timer.lock(|timer| {
                timer.start(2.micros()).unwrap();
                timer.listen(Event::Update);
            });
        } else if ctx.local.stop_button.check_interrupt() {
            rprintln!("Stop button");
            ctx.local.stop_button.clear_interrupt_pending_bit();

            ctx.shared.bullet_timer.lock(|timer| {
                timer.unlisten(Event::Update);
            });

            ctx.shared.counter.lock(|couner| {
                let seconds = *couner as f32 * 0.000_002;
                rprintln!("seconds: {}", seconds);
                *couner = 0;
            });
        }
    }

    #[task(
        binds = TIM1_UP,
        priority = 2,
        local=[tick_timer, led]
    )]
    fn tick(ctx: tick::Context) {
        ctx.local.tick_timer.clear_interrupt(Event::Update);
        ctx.local.led.toggle();
    }

    #[task(binds = TIM2, priority = 1, shared=[counter, bullet_timer])]
    fn increase_counter(mut ctx: increase_counter::Context) {
        ctx.shared
            .bullet_timer
            .lock(|timer| timer.clear_interrupt(Event::Update));

        ctx.shared.counter.lock(|counter| {
            *counter += 1;
        })
    }
}
