use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{AnyInputPin, Input, InterruptType, PinDriver, Pull},
    task::notification::Notification,
};
use std::num::NonZero;

pub struct UserInput {
    notification: Notification,
    button: PinDriver<'static, Input>,
}

impl UserInput {
    pub fn new(pin: AnyInputPin<'static>) -> anyhow::Result<Self> {
        let mut button = PinDriver::input(pin, Pull::Down)?;
        button.set_interrupt_type(InterruptType::PosEdge)?;
        let notification = Notification::new();
        let waker = notification.notifier();
        unsafe {
            button.subscribe_nonstatic(move || {
                waker.notify_and_yield(NonZero::<u32>::MIN);
            })?;
        }
        button.enable_interrupt()?;
        Ok(Self {
            notification,
            button,
        })
    }

    pub fn wait_for_input(&mut self) -> anyhow::Result<()> {
        self.notification.wait_any();
        // Debounce
        FreeRtos::delay_ms(200);
        self.button.enable_interrupt()?;
        Ok(())
    }
}
