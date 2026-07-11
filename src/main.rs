// Школьный проект на Ariel OS
use ariel_school::{
    school_main, digital_write,
    delay, LED_BUILTIN, HIGH, LOW
};

#[school_main]
fn setup() {}

fn loop_tick() {
    digital_write(LED_BUILTIN, HIGH);
    delay(500);
    digital_write(LED_BUILTIN, LOW);
    delay(500);
}