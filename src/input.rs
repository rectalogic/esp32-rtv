use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{AnyInputPin, Input, InterruptType, PinDriver, Pull},
    task::notification::Notification,
};
use std::{
    num::NonZero,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy)]
pub enum ButtonEvent {
    Pressed,
    Released { held: Duration },
}

pub struct UserInput {
    notification: Notification,
    button: PinDriver<'static, Input>,
    pressed_at: Option<Instant>,
}

impl UserInput {
    // Longer than switch bounce (~5-20ms)
    const DEBOUNCE_MS: u32 = 30;

    pub fn new(pin: AnyInputPin<'static>) -> anyhow::Result<Self> {
        let mut button = PinDriver::input(pin, Pull::Up)?;
        button.set_interrupt_type(InterruptType::AnyEdge)?;
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
            pressed_at: None,
        })
    }

    pub fn wait_for_event(&mut self) -> anyhow::Result<ButtonEvent> {
        loop {
            self.notification.wait_any();

            FreeRtos::delay_ms(Self::DEBOUNCE_MS);

            let event = if self.button.is_low() {
                ButtonEvent::Pressed
            } else {
                ButtonEvent::Released {
                    held: self.pressed_at.map_or(Duration::ZERO, |t| t.elapsed()),
                }
            };

            self.button.enable_interrupt()?;

            match event {
                ButtonEvent::Pressed if self.pressed_at.is_none() => {
                    self.pressed_at = Some(Instant::now());
                    return Ok(event);
                }
                ButtonEvent::Released { .. } => {
                    self.pressed_at = None;
                    return Ok(event);
                }
                ButtonEvent::Pressed => {
                    // Already tracked as pressed; ignore the spurious edge.
                }
            }
        }
    }
}
