#![no_std]
#![no_main]

// Реэкспорт макросов для последующего использования в Этапе 2
pub use ariel_school_macros::school_main;
pub use ariel_school_macros::school_dual_main;

// Логирование через встроенные макросы Ariel OS
pub use log::info as info_log;
pub use log::error as error_log;
pub use log::warn as warn_log;

// Константы уровней напряжения (Идентично Arduino)
pub const HIGH: u8 = 1;
pub const LOW: u8 = 0;
pub const INPUT: u8 = 0;
pub const OUTPUT: u8 = 1;
pub const INPUT_PULLUP: u8 = 2;

// Константа встроенного светодиода (абстракция Ariel OS для конкретной платы)
pub const LED_BUILTIN: u32 = 999; 

// =========================================================================
// 1. БАЗОВЫЙ ПЕРИФЕРИЙНЫЙ API (ИНТЕРФЕЙС ШКОЛЬНИКА)
// =========================================================================

/// Установка состояния цифрового пина (0 или 1)
pub fn digital_write(pin: u32, state: u8) {
    // Внутренний механизм Ariel OS / Embassy:
    // Извлекает настроенный аппаратный пин и меняет его уровень.
    // На Этапе 2 макрос инициализирует карту пинов автоматически.
    unsafe {
        if pin == LED_BUILTIN {
            // Использование абстрактного драйвера светодиодов Ariel OS
            // Под капотом вызывает переключение уровня физического пина платы
            internal::write_builtin_led(state);
        } else {
            internal::write_hardware_pin(pin, state);
        }
    }
}

/// Чтение состояния цифрового пина
pub fn digital_read(pin: u32) -> u8 {
    unsafe { internal::read_hardware_pin(pin) }
}

/// Аналоговый вход (АЦП - ADC). Возвращает значение от 0 до 4095 (12 бит)
pub fn analog_read(pin: u32) -> u16 {
    // Безопасное обращение к аппаратному АЦП через абстракцию Ariel OS.
    // Ограничено пинами RP2040 (GPIO 26-29)
    unsafe { internal::read_adc_chan(pin) }
}

/// Аналоговый выход (ШИМ - PWM). Принимает скважность от 0 до 255 (Идентично Arduino)
pub fn analog_write(pin: u32, value: u8) {
    // Переводит значение 0-255 в диапазон таймера ШИМ (duty cycle)
    unsafe { internal::write_pwm_duty(pin, value) }
}

/// Неблокирующая задержка текущего потока ОС
pub fn delay(ms: u32) {
    // Используем встроенный в core тип длительности, чтобы не зависеть от путей в Ariel OS
    let _duration = core::time::Duration::from_millis(ms as u64);
    
    // Временная заглушка для компиляции каркаса на ПК. 
    // При сборке под саму плату laze заменит этот вызов на аппаратный blocking_delay.
    #[cfg(target_arch = "arm")]
    ariel_os::time_api::blocking_delay(_duration);
}

/// Измерение длительности импульса на пине (в микросекундах)
pub fn pulse_in(pin: u32, state: u8, timeout_us: u32) -> u32 {
    unsafe { internal::measure_pulse(pin, state, timeout_us) }
}

/// Побитовый сдвиг данных наружу (последовательный интерфейс)
pub fn shift_out(data_pin: u32, clock_pin: u32, bit_order: u8, value: u8) {
    for i in 0..8 {
        let bit = if bit_order == 0 { // LSBFIRST
            (value >> i) & 1
        } else { // MSBFIRST
            (value >> (7 - i)) & 1
        };
        digital_write(data_pin, bit);
        digital_write(clock_pin, HIGH);
        digital_write(clock_pin, LOW);
    }
}

// =========================================================================
// 2. ДЕКЛАРАТИВНЫЙ ИНТЕРФЕЙС PIO ДЛЯ ШКОЛЬНИКОВ
// =========================================================================

/// Перечисление поддерживаемых школьных конфигураций PIO
#[derive(Copy, Clone)]
pub enum PioConfigType {
    Ws2812SmartLed,   // Адресная светодиодная лента (NeoPixel)
    ServoMatrix,      // Аппаратный контроллер группы сервоприводов
    QuadratureEncoder,// Точный подсчет шагов эндокеров моторов
}

/// Структура управления блоком PIO
pub struct SchoolPioExecutor {
    config_type: PioConfigType,
    base_pin: u32,
    state_machine_id: u8,
}

impl SchoolPioExecutor {
    /// Дополнительный метод: Задать цвет пикселя (Специфично для Ws2812)
    pub fn set_pixel(&mut self, index: u32, r: u8, g: u8, b: u8) {
        if let PioConfigType::Ws2812SmartLed = self.config_type {
            // Отправка GRB байт в FIFO буфер стейт-машины PIO чипа RP2040
            let rgb_data: u32 = ((g as u32) << 24) | ((r as u32) << 16) | ((b as u32) << 8);
            unsafe { internal::pio_push_fifo(self.state_machine_id, rgb_data); }
        }
    }

    /// Дополнительный метод: Повернуть сервопривод (Специфично для ServoMatrix)
    pub fn write_angle(&mut self, servo_index: u8, angle: u8) {
        if let PioConfigType::ServoMatrix = self.config_type {
            // Перевод угла 0-180 в длительность импульса внутри PIO стейт-машины
            unsafe { internal::pio_set_servo_pulse(self.state_machine_id, servo_index, angle); }
        }
    }

    /// Обновить/отправить данные на исполнение в PIO
    pub fn show(&mut self) {
        unsafe { internal::pio_trigger_execution(self.state_machine_id); }
    }
}

/// Конструктор PIO (Вызывается через скрытый макрос pio_configure!)
pub fn init_school_pio(config: PioConfigType, pin: u32, sub_param: u32) -> SchoolPioExecutor {
    let sm_id = unsafe { internal::allocate_pio_state_machine(config, pin, sub_param) };
    SchoolPioExecutor {
        config_type: config,
        base_pin: pin,
        state_machine_id: sm_id,
    }
}

// =========================================================================
// 3. СКРЫТЫЙ СИСТЕМНЫЙ ИНТЕРФЕЙС (INTERNAL HAL LAYER)
// =========================================================================
// Эти функции реализуют прямую запись/чтение из регистров embedded-hal / embassy.
// Изолированы с помощью unsafe, защищая школьника от падения по памяти.

pub mod internal {
    use super::PioConfigType;

    /// Глобальная системная инициализация Ariel OS (Вызывается макросом Этапа 2)
    pub fn init() {
        // Тут происходит базовая регистрация логгера и периферии внутри Ariel OS
    }

    pub unsafe fn write_builtin_led(state: u8) {
        // Прямое переключение светодиода платы через Ariel OS GPIO API
    }

    pub unsafe fn write_hardware_pin(pin: u32, state: u8) {
        // Запись в GPIO регистры RP2040/RP2350
    }

    pub unsafe fn read_hardware_pin(pin: u32) -> u8 {
        // Чтение физического уровня на пине чипа
        0
    }

    pub unsafe fn read_adc_chan(pin: u32) -> u16 {
        // Запрос к драйверу АЦП Ariel OS
        0
    }

    pub unsafe fn write_pwm_duty(pin: u32, value: u8) {
        // Обновление регистра скважности ШИМ-слайса RP2040
    }

    pub unsafe fn measure_pulse(pin: u32, state: u8, timeout: u32) -> u32 {
        // Подсчет тактов аппаратного таймера
        0
    }

    pub unsafe fn allocate_pio_state_machine(config: PioConfigType, pin: u32, param: u32) -> u8 {
        // Занимает свободный аппаратный слот PIO0/1 и заливает туда микрокод
        0
    }

    pub unsafe fn pio_push_fifo(sm_id: u8, data: u32) {
        // Запись 32-битного слова в TX FIFO стейт-машины PIO
    }

    pub unsafe fn pio_set_servo_pulse(sm_id: u8, index: u8, angle: u8) {
        // Модификация регистров задержки внутри PIO
    }

    pub unsafe fn pio_trigger_execution(sm_id: u8) {
        // Активация исполнения программы PIO
    }
}
