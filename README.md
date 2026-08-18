**STEM**  **Rustino** 

# The Rationale for a Macro-Build on  **Rust** 

**How is Ariel OS superior to Arduino + FreeRTOS?**

**1. Memory safety without sacrificing speed**

In Arduino C++, any pointer error (out-of-bounds, use-after-free) in the motor or sensor control code is guaranteed to crash the flight controller or lead to unpredictable behavior.  **Rust guarantees 100% memory safety at the compiler level**  . Furthermore, Ariel OS doesn't use a heavy virtual machine (like MicroPython), maintaining the speed of bare metal. 

**2. Asynchronicity (async/await) out of the box**

In traditional FreeRTOS, multithreading is built on preemptive tasks, which require allocating a fixed stack of memory for each thread (e.g.,  xTaskCreate(..., stack_size=2048)  ). If there was a stack error, the controller would hard fault. 

- **Ariel OS combines classic threads and an asynchronous approach (async/await)**  . Thousands of asynchronous tasks can be efficiently executed on a single ESP32-C3 core, dynamically sharing memory and without the risk of individual task stack overflows. 

**3. True code portability (HAL-agnostic)**

The Arduino ecosystem has a problem: sensor libraries are often written for a specific architecture (AVR or ESP). Ariel OS uses  embedded-hal abstractions  . By writing code to access a gyroscope via I2C on an ESP32-C3, you can compile it for an STM32, Raspberry Pi Pico, or Nordic nRF52 without changing a single line of logic. 

**4. No hidden "garbage" (Boilerplate)**

Unlike ESP-IDF, Ariel OS is a "library OS." The meta-build system (  laze  ) automatically configures drivers, network stacks, debugging (  defmt  ), and firmware via  probe-rs  using simple flags. The code is concise. 

---

**The downside: How will Ariel OS be less convenient?**

|**Criterion**|**Arduino IDE + FreeRTOS**|**Ariel OS (Rust)**|
|:---:|:---:|:---:|
|**Entry threshold**|Low. Millions of examples in C/C++.|High. Requires a deep understanding of Rust and async.|
|**Ready-made libraries**|There is one for absolutely everything (IMU, motors, screens).|The Rust driver ecosystem is growing, but specific libraries will have to be written from scratch.|
|**ESP peripheral support**|Full-featured (Wi-Fi, BLE, hardware PWM, ADC).|It's still evolving. The basics (GPIO, UART, I2C, Wi-Fi) work, but BLE, for example, on the ESP series is still in the process of stabilizing.|

For  DIY  projects  It's difficult  to use Ariel OS without an add-on: 

1. Arduino's control algorithms  are deeply tied to C++ math libraries and ready-made sensor drivers. Porting this code to Rust will take months. 

2. Active control projects require precise and fast PWM control for motors, which has been honed over the years in Arduino for the ESP32 using hardware interrupts. In Ariel OS for the ESP32-C3, setting up these low-level timers requires manual register manipulation via  esp-hal  . 

# Project  **Rustino** 

**Rustino**  project  can solve the described problem at the ecosystem level. 

Instead of trying to reconcile incompatible C++ and Rust binaries, we propose  to port the declarative spirit and syntactic sugar of Arduino to the Rust compiler and Ariel OS  **.** 

Splitting the architecture into macros (  ariel_school_macros  ) and the environment itself (  school_ide  /  ariel_school  ) can help solve the "difficult Rust" problem for beginners: 

**1. Procedural Macros as a Boilerplate Rescue**

Embedded Rust (especially asynchronous on Embassy/Ariel) usually intimidates newbies with its abundance of attributes like  #![no_std]  ,  #![no_main]  , peripheral initialization  esp_hal::init(...) and complex  main  function signature  . 

Using macros in  ariel_school_macros  , you can hide all this complexity. The user sees the familiar structure, but behind the scenes, the compiler unfolds it into valid, strict, and safe Rust code. 

**2. Safe implementation of asynchronous**  **delay()** 

In traditional Rust for ESP, you would have to pass a timer instance or  Delay  structure through function arguments or create a global singleton. Encapsulating time management within  ariel_school  allows you to write a concise  delay(500)  , which, under the hood, effectively invokes a non-short-circuiting asynchronous sleep in Ariel OS, freeing the ESP32-C3 processor for background networking tasks. 

**3. Compile-time security**

The coolest thing about Rustino is that if a student or developer makes a typo, tries to use a pin already occupied by another sensor, or goes beyond the bounds of an array,  **the Rust compiler will throw an error during the build process in the IDE**  . In classic Arduino C++, such code would compile, but the drone would simply fly into a wall due to  a segmentation fault  or memory corruption during flight. 

# **SYSTEM SPECIFICATION AND GUIDE: RUSTINO ECOSYSTEM**

**Base engine:**  Ariel OS (based on Rust / Embassy) 

**Supported target platforms:**  ESP32-C3, RP2040, RP2040W (with future seamless expansion) 

---

# PART 1. Architectural philosophy and concept

**Rustino**  is a cross-platform declarative framework (Library OS) built on top of  **the Ariel OS real-time operating system**  . 

## **The main task of the ecosystem**

Provide the developer with an intuitive, linear "Arduino-like" syntax (C-style) for rapid device development, but compile it to  **100% safe, asynchronous, and energy-efficient Rust code**  . 

## The Architectural Foundations of Rustino

1. **Hardware Agnostic:**  The same user sketch can be compiled for  **RISC-V (ESP32-C3)**  or  **ARM Cortex-M0+ (RP2040 / RP2040W) architecture chips**  . All low-level peripheral driver substitution (  esp-hal  ,  rp2040-hal  , etc.) is isolated within system macros. 

2. **Zero Boilerplate:**  The user does not see or write  #![no_std]  ,  #![no_main]  attributes,  unsafe blocks  , lifetime markers (  'a  ), and complex asynchronous constructs like  async/await  . 

3. **Asynchrony disguised as synchronicity:**  Timing functions (e.g.,  delay()  ) appear as linear sequential code, but under the hood, they unfold into non-blocking  .await system calls  . This allows the Ariel OS scheduler to seamlessly and concurrently handle networking stacks (Wi-Fi on the ESP32-C3 and RP2040W, Bluetooth) and background processes. 

4. **Compile-Time Validation:**  Resource allocation is verified at compile time. Attempting to use the same pin in conflicting modes (for example, simultaneously as a digital output and as part of the I2C bus) will result in a build error, protecting the device from runtime crashes. 

---

# PART 2. Complete API Syntax Reference

Custom code is written inside a module marked with the main system macro -  #[rustino::sketch]  . 

## 2.1 Initialization and Entry Points

Required for any project are two functions that accept a reference to the system context  Context  (abbreviated  ctx  ): 

- fn setup(ctx: &mut Context)  — Called once at controller startup. Used to configure pins, sensors, and interfaces. 

- fn loop(ctx: &mut Context)  — Called cyclically by the scheduler. 

## 2.2. Input/Output Control (GPIO and ADC)

Access to the chip's physical pins is strictly through the  ctx.pinout structure  . The specific pin names depend on the selected target board, but the methods for configuring them are standard. 

- **Configuration of operating modes:**

rust

let  led = ctx.pinout.led_onboard.into_output();  //  Digital  exit 

let  button = ctx.pinout.gpio3.into_input();  //  Digital  entrance 

let  sensor = ctx.pinout.adc0.into_analog();  //  Analog  input  (  ADC  ) 



- **Digital I/O:**

rust

led.high();  // Apply power (3.3V) 

led.low();  // Turn off the power (0V) 

led.toggle();  // Invert the current state of the pin 

let  is_pressed: bool = button.is_high();  //  Count  logical  level 



- **Analog Input:**

rust

\// Returns u16. The range depends on the target chip's ADC resolution:

\// 0..4095 for 12-bit ADC (ESP32-C3, RP2040)

let  raw_value = sensor.read_analog(); 



## 2.3. Time and Task Dispatching

- delay(ms: u32)  — Asynchronous pause. Pauses execution of the current function for the specified number of milliseconds, freeing up the processor for other tasks and background computations. 

- pass()  — Micro-pause (Yield). This function doesn't sleep, but takes a forced short step back, allowing Ariel OS to check the system task queue (for example, processing Wi-Fi packets), and then immediately returns. This function is used inside heavy math loops. 

## 2.4. Debug Data Output (Serial)

Global interface for outputting text information to the port monitor.

- Serial.begin(baud_rate: u32)  — Interface initialization (standard: 115200). 

- Serial.print!("text {} ", x)  — Zero-allocation stream output macro. Accepts string literals and formatting arguments. 

- Serial.print_line!("text {} ", x)  — A stream output macro with automatic line breaks. Formats data on the fly directly to the port, without using the heap (  String  ) and the heavy  format!() macro  . 

---

## 2.5. Universal I2C bus

- **Interface initialization:**

rust

let  wire = ctx.i2c.init(ctx.pinout.sda_pin, ctx.pinout.scl_pin); 



- **Capture Peripheral Device:**

rust

let  mut  imu = wire.connect_device(  0x68  );  // Connect to the sensor using its I2C address 



---

# PART 3. Advanced Asynchronous and Event-Driven Functionality

Rustino extends the classic Arduino paradigm by introducing a sophisticated event-driven model without complicating the syntax.

## 3.1 Cross-functional events and triggers

Allow isolated functions to react instantly to each other's actions without using expensive global flags in a loop  ()  . 

- #[event_channel]  — An attribute for declaring a communication channel at the module level. There are two types: 

  - Trigger  — A simple signal ("An event has occurred"). Activated by the  .fire() method  . 

  - Event<T>  — A signal carrying a  T data structure  . Activated by the  .emit(data) method  . 

- #[on_event(channel_name)]  — An attribute above a function. Turns it into an independent Ariel OS task that is in deep sleep mode and wakes up at the system level  **only**  when a signal is sent to the specified channel. 

## 3.2 Declarative Timers

- #[every(period)]  — An attribute that automatically turns a function into a cyclic background task. Supports time unit suffixes:  ::ms  (milliseconds) and  ::s  (seconds). 

## 3.3 Cross-platform thread synchronization macros

**Task race (**  **rustino::race!**  **)** 

Runs multiple blocks of code in parallel. The block that completes first wins the race, and the remaining blocks  **are immediately and safely destroyed**  . Ideal for implementing hard peripheral timeouts. 

rust

rustino::race! {

success => {

let data = imu.read_data(); // Wait for the sensor response

    }, 

    timeout  => { 

        delay  (5); // The operation execution time limit is 5 milliseconds 

        Serial  .  print  _  line  !("Error: Sensor timeout!"); 

\}

\}

**Merge tasks (**  **rustino::join!**  **)** 

Runs all listed code blocks simultaneously and suspends execution of the current thread until  **each**  block completes its work. Ideal for parallel polling of independent slow sensors (e.g., GPS and barometer). 

rust

rustino::join! {

\{ gps.update_coordinates(); },

    { baro.read_pressure(); } 

\}

\// The code will go here only after both blocks have completed successfully.



---

# PART 4. Cross-platform tutorial (Implementation examples)

## Lesson 1: Responsive Blink (Supporting Three Platforms in One Code)

The example demonstrates the use of conditional compilation  #[cfg]  to automatically detect the onboard LED depending on which controller the project is currently being compiled for. 

rust

\#[rustino::sketch]

mod  adaptive_blink { 

    let  mut  led: Pin; 



    fn  setup(ctx: &  mut  Context) { 

        Serial.begin(  115200  ); 



        // Automatic pin selection at compile time 

        #[cfg(feature = "esp32c3")] { 

led = ctx.pinout.gpio8.into_output(); // LED  boards  ESP32-C3 Super Mini 

Serial.print_line!("  Startup  : RISC-V  Architecture  (ESP32-C3)"); 

\}

\#[cfg(feature = "rp2040")] {

led = ctx.pinout.gpio25.into_output(); // LED  of the original  Raspberry Pi Pico 

Serial.print_line!("  Launch  : ARM  Architecture  (RP2040)"); 

\}

\#[cfg(feature = "rp2040w")] {

//  On the  RP2040W  LED  connected  To  Wi-Fi  chip (wl_gpio0) 

led = ctx.pinout.wifi_led.into_output();

Serial.print_line!("  Startup  :  ARM  architecture  with  Wi-Fi (RP2040W)"); 

\}

    fn  loop  (ctx: &  mut  Context) { 

        led.high(); 

delay(  200  );  // Asynchronous sleep frees up the core on all processor types 

led.low();

delay(  200  ); 

\}

\}



## Lesson 2: Flight Controller Architecture (Flix Project Template)

An example of decomposing a real drone into isolated, asynchronous, event-interacting circuits.

rust

\#[rustino::sketch]

mod flix_flight_control {

use drone_library::{Angles, MotorOutputs};



    // Defining the module's hardware resources 

    let  mut  status  _  led  :  Pin  ; 

    let mut speed_controllers: QuadMotors; 

let  mut  imu  :  I  2  cDevice  ; 



\// GENERATION OF INTERFUNCTIONAL COMMUNICATION CHANNELS

    #[event_channel] 

let telemetry_channel: Event<Angles>; // Channel for transmitting spatial angles

    

\#[event_channel]

let emergency_signal: Trigger; // Instant stop emergency trigger



    // 1. INITIAL SETUP 

    fn  setup  (  ctx  : &  mut  Context  ) { 

        Serial  .  begin  (115200); 

        

\// Universal cross-platform status LED binding

        #[cfg(feature = "esp32c3")] { status_led = ctx.pinout.gpio8.into_output(); } 

\#[cfg(feature = "rp2040")] { status_led = ctx.pinout.gpio25.into_output(); }

\#[cfg(feature = "rp2040w")] { status_led = ctx.pinout.wifi_led.into_output(); }



        // Initialize the  I  2  C hardware bus  for the gyroscope 

        let  wire  =  ctx  .  i  2  c  .  init  (  ctx  .  pinout  .  sda  _  default  ,  ctx  .  pinout  .  scl  _  default  ); 

        imu = wire.connect_device(0x68); 



        // Initialization of PWM power keys for controlling 4 motors 

        speed_controllers = ctx.pwm.init_quad_linear_motors(); 

        

Serial.print_line!("Rustino Core System for Flix: READY");

    } 



\// 2. HIGH-FREQUENCY POLLING CYCLE (Frequency 100 Hz)

    fn loop(ctx: &mut Context) { 

        // We control  the  sensor reading: if the  I2C  bus  is shorted, the drone will not hover in the air 

        rustino::race! { 

sensor_ready => {

let current_angles = imu.read_gyro_angles();

                

if current_angles.is_critical_crash() {

emergency_signal.fire(); // Initiate an immediate stop

\} else {

telemetry_channel.emit(current_angles); // Send data to the PID loop

                } 

\},

            sensor_fault  =  >  { 

                delay  (2); // If the sensor has not responded within 2 ms, we abort the operation. 

                Serial  .  print  _  line !("Warning  :  I2C  data  bus jitter or failure  !"); 

\}

\}



        delay  (10); // Strict iteration step of the main loop is 10 ms 

\}



\// 3. STABILIZATION CIRCUIT (Asynchronous telemetry event handler)

// The function is completely asleep and is activated only when  .emit  () is called 

// Uses  SubContext  to bypass  Borrow restrictions  Checker 

    #[on_event(telemetry_channel)] 

fn process_pid_loop(ctx: &mut SubContext, angles: Angles) {

let calculated_pwm = flix_math_pid(angles);

speed_controllers.apply_power(calculated_pwm);

    } 



\// 4. SAFETY CIRCUIT (Highest Priority Emergency Trigger)

// Uses  SubContext  to bypass  Borrow restrictions  Checker 

    #[on_event(emergency_signal)] 

fn execute_emergency_shutdown(ctx: &mut SubContext) {

speed_controllers.kill_all_power ();  //  Physically  de  -  energize  the  motor  power  transistors 

        Serial.print_line!("CRITICAL: Drone flipped over.  Motors are disabled!"); 



\// We put the processor into an infinite loop indicating a critical failure

        loop { 

status_led.toggle();

delay(50);

        } 

\}



\// 5. PERIODIC MONITORING (Works autonomously every 2 seconds)

\// Does not introduce delays into motor stabilization cycles

// Uses  SubContext  for parallel access to peripherals 

    #[every(2::s)] 

fn send_diagnostic_packet(ctx: &mut SubContext) {

        // If the packet sending procedure is heavy (for example,  the Wi  -  Fi stack  on  the RP  2040  W  ), 

//  the pass  () function ensures that the sending will be divided into time quanta, 

\// without causing hardware starvation of other threads

        network_push_telemetry_bytes(); 

pass();

\}

\}

---

# PART 5. Instructions-Context for AI assistants and Viber coders

*When specifying any language model (LLM) task for code generation within this project, be sure to attach the following text as a system constraint:*

You are a specialized embedded systems coder in the Rustino ecosystem (a high-level abstraction over the asynchronous Ariel OS). Your task is to generate code strictly according to the following rules:



1. **Global structure:** All user code must be encapsulated inside a declarative module with the main macro: `#[rustino::sketch] mod project_name { ... }`.

2. **Low-level syntax taboo:** It is strictly forbidden to manually use the keywords `async`, `await`, `unsafe`, as well as the root directives `#![no_std]` or `#[no_main]`. They are expanded automatically by the framework macro.

3. **Module global variables:** Virtual global objects (pin, sensor, and interface structures) are declared using the `let mut` keyword directly at the root of the sketch module (e.g., `let mut led: Pin;`). Using the standard `static` or `static mut` constructs is prohibited. Use regular Rust local variables inside the `setup` and `loop` functions.

4. **Entry points:** The mandatory functions are `fn setup(ctx: &mut Context)` and `fn loop(ctx: &mut Context)`. Any interaction with the hardware is performed exclusively through the internal methods of the passed `ctx` context. Using raw numeric pin indices (as in Arduino) is prohibited.

5. **Timing:** To put the thread to sleep, use only the global function `delay(ms: u32);` (e.g. `delay(500);`). To prevent scheduler starvation inside heavy math loops, call the processor yield function `pass();`.

6. **Cross-platform:** To write code for different chips (ESP32-C3, RP2040 and RP2040W), use Rust conditional compilation blocks: `#[cfg(feature = "esp32c3")]`, `#[cfg(feature = "rp2040")]` and `#[cfg(feature = "rp2040w")]`. Note that on the RP2040W the onboard LED is initialized as `ctx.pinout.wifi_led`, on the RP2040 as `ctx.pinout.gpio25`, and on the ESP32-C3 as `ctx.pinout.gpio8`.

7. **Event Model:** It is prohibited to build parallel logic based on manually calculating the difference in system time (flags like `millis()`). Use asynchronous event abstractions: `#[event_channel]` for declaring variables (of `Trigger` or `Event<T>` types), `.fire()` or `.emit(data)` methods for non-blocking signal sending, and the `#[on_event(channel)]` attribute above handler functions.

8. **Background timers:** For cyclic tasks, use the `#[every(time::unit)]` attribute on isolated functions (e.g. `#[every(1::s)]` or `#[every(100::ms)]`).

9. **Callback Function Signatures:** Functions marked `#[on_event(...)]` and `#[every(...)]` must accept `ctx: &mut SubContext` as their first argument. The `SubContext` type is a thread-safe proxy object that allows the runtime to share access to hardware resources between parallel tasks without violating Borrow Checker rules.

10. **Synchronization and fault tolerance:** To prevent firmware from freezing due to external sensor failures, always wrap interface reading in the `rustino::race!` timeout macro. To execute tasks simultaneously in parallel, use the `rustino::join!` macro.

11. **Data Output (Serial):** For debugging, use the global macros `Serial.print!` and `Serial.print_line!`. Port initialization: `Serial.begin(115200);`. Text and variable output is performed similarly to the standard println! in Rust, but without memory allocation: `Serial.print_line!("Telemetry: X={}, Y={}", ax, ay);`. Using the `String` type, the `.to_string()` method, and the `format!()` macro is strictly prohibited (the system operates in strict zero-allocation stream output mode). Use only string literals like `&str`.

12. **Modularity:** To move logic into a separate file (e.g. `math_utils.rs`), declare it within the sketch as `mod math_utils;` and access its functions via `math_utils::function_name()`. All external libraries (crates) are included via `Cargo.toml`, and their components are imported using the standard `use` directive.

13. **SPI bus and custom UART:**

\- To work via SPI, first initialize the shared bus with `let bus = ctx.spi.init(sck, mosi, miso);`, and then connect the device with `let mut dev = bus.connect_device(cs_pin, frequency_hz);`. Data exchange is performed using the `dev.transfer(&tx_buf, &mut rx_buf);` method.

\- To connect external devices via UART, use `ctx.uart.init(tx, rx, baud);`. Reading and writing are performed via the non-blocking `.read_byte()` and `.write_byte()` methods.

---

# PART 6. Build Infrastructure (Configuration Files)

To create  **a complete and seamless repository**  that can be immediately released for development,  **an infrastructure configuration (Build and Packages) is required**  . 

Without properly configuring the  Cargo.toml files  and the  laze configurator  , the Rust compiler simply won't understand how to compile syntactic sugar for three different architectures (RISC-V for ESP, ARM for RP). 

Below are the project configuration files that tie Ariel OS, macros, and code together.

These files should be located in the root of  the school_ide  /  Rustino project  . They control which drivers (features) are included during compilation. 

## Cargo.toml  File  (For managing Rust dependencies) 

toml

\[package]

name =  "rustino_project" 

version =  "1.0.0" 

edition =  "2021" 

authors = [  "Yuriy Shved <  your  _  email  >"  ] 



\[dependencies]

\# Connecting the Ariel OS kernel (versions for different architectures)

ariel-os = { version =  "0.1"  , features = [  "storage"  ,  "time"  ] } 



\# Crate with your procedural macros that transform the sketch

ariel_school_macros = { path =  "../ariel_school_macros"  } 

ariel_school = { path =  "../ariel_school"  } 



#  Auxiliary  libraries  For  logging  debugging 

esp-backtrace = { version =  "0.12"  , optional =  true  } 



\[features]

\# Platform flags used in the sketch via #[cfg(feature = "...")]

esp32c3 = [  "ariel-os/esp32c3"  ,  "dep:esp-backtrace"  ] 

rp2040 = [  "ariel-os/rp2040"  ] 

rp2040w = [  "ariel-os/rp2040"  ,  "ariel-os/wifi"  ]  #  Connects  Wi-Fi  stack  for  Raspberry Pi 



\[profile.release]

opt-level =  "z"      # Maximum optimization for binary size (critical for Flix) 

lto =  true           # Link-Time Optimization to remove unused code from Arduino libraries 

codegen-units =  1    # Increases the efficiency of code optimization by the compiler 

panic =  "abortion"      # In case of panic, simply reboot the controller 



## laze-project.yml  file  (Ariel OS builder configurator) 

Ariel OS uses the  laze utility  to automatically build network stacks and the HAL. This file specifies how to flash various chips. 

yaml

\# Rustino project build configuration

laze  : 

  extends  : 

\- ariel-os



modules  : 

-  name  : rustino_app 

    type  : app 

    srcs  : 

      - src/main.rs  # Here is the user sketch #[rustino::sketch] 

    depends  : 

\- ariel-os-core



\# Presets for quick launch with one command

targets  : 

-  name  : flix-esp32c3 

    select  : 

-  board  : esp32c3-super-mini  #  Name  boards  in  Ariel OS 

    env  : 

      RUSTFLAGS  :  "--cfg feature=\"esp32c3\"" 



-  name  : pico-rp2040 

    select  : 

-  board  : raspberry-pi-pico 

    env  : 

      RUSTFLAGS  :  "--cfg feature=\"rp2040\"" 



-  name  : pico-rp2040w 

    select  : 

-  board  : raspberry-pi-pico-w 

    env  : 

      RUSTFLAGS  :  "--cfg feature=\"rp2040w\"" 



---

# PART 7. Cheat Sheet for Launch Commands (For Developers)

To compile and upload a Rustino sketch to the controller, simple system commands are used, for example:

- **Building and flashing the Flix drone on the ESP32-C3:**

bash

laze build -t flix-esp32c3 flash



- **Assembling and flashing the RP2040 (Raspberry Pi Pico) board:**

bash

laze build -t pico-rp2040 flash



- **Assembly for RP2040W (with Wi-Fi support):**

bash

laze build -t pico-rp2040w flash



---

# PART 8. Modularity and External File Inclusion in Rustino

The framework supports three levels of code separation:

1. **Internal Modules (Project Subfiles):**  Splitting a single large sketch into logical files (e.g.  pid.rs  ,  motors.rs  ). 

2. **Local Libraries (Code Folders):**  Create your own reusable drivers inside your project folder. 

3. **External Ecosystem Libraries (Crates):**  Connecting ready-made libraries from the global Rust repository (  crates.io  ). 

---

## 8.1. Including local files (analogous to  .h  and  .cpp  files) 

To move a section of code (for example, PID controller math) into a separate file, the user simply needs to use the standard Rust  mod keyword  . 

**Step 1. Create the**  **src/pid.rs file**  **(Logic file)** 

This file contains plain, pure Rust code. It doesn't require the  #[rustino::sketch] macro  . 

rust

\// src/pid.rs

\// The `pub` keyword makes the structure and functions visible to the main sketch



pub  struct  PidController { 

p_gain: f32,

i_gain: f32,

d_gain: f32,

integral: f32,

\}



impl  PidController { 

    pub  fn  new(p: f32, i: f32, d: f32) ->  Self  { 

        Self  { p_gain: p, i_gain: i, d_gain: d, integral:  0.0  } 

\}



    pub  fn  calculate(&  mut  self  , target: f32, current: f32) -> f32 { 

        let  error = target - current; 

        self  .integral += error; 

        //  Simplified  formula  For  example 

(error *  self  .p_gain) + (  self  .integral *  self  .i_gain) 

    } 

\}



**Step 2. Connect it to the main sketch**  **src/main.rs** 

rust

\// src/main.rs

\#[rustino::sketch]

mod  flix_drone { 

    //  ANALOGUE  #include "pid.h" 

    // Macro allows using `mod` keyword inside sketch 

    mod  pid; 



    // Using the structure from the included file 

    let  mut  pitch_regulator: pid::PidController; 



    fn  setup(ctx: &  mut  Context) { 

Serial.begin(  115200  ); 

        

        //  Initialize  regulator 

pitch_regulator = pid::PidController::new(  1.2  ,  0.05  ,  0.1  ); 

\}



fn  loop  (ctx: &  mut  Context) { 

let  current_angle = ctx.imu.read_pitch(); 

    let  power = pitch_regulator.calculate(  0.0  , current_angle); 

        //  Manage  motors  ... 

        pass(); 

\}

\}



---

## 8.2 How the  #[rustino::sketch] macro  handles external files 

When a user types  mod pid; within a sketch, the Rust compiler looks for a  pid.rs  file by default .  *inside*  the current module, which would cause a build error. 

**How the macro works under the hood:**

The ariel_school_macros  procedural macro finds all  mod name;  declarations when parsing a module  . It automatically moves them  **outside**  the user sketch to the root of the  main.rs file  , transforming the code into a structure valid for the Rust compiler: 

rust

\// What the macro turns the code into before compilation:

\#![no_std]

\#![no_main]



\// The macro moved the module outside so that Rust could find the src/pid.rs file.

mod  pid; 



\#[ariel_os::main]

async  fn  main() -> ! { 

    // System initialization... 

    

    // Inner call to sketch where pid is now accessible via root namespace 

    let  mut  pitch_regulator =  crate  ::pid::PidController::new(  1.2  ,  0.05  ,  0.1  ); 

    

    loop  {  /* ... */  } 

\}



---

## 8.3. Connecting external libraries (analog of the Arduino library manager)

If your project requires a ready-made library, you don't need to download its files manually. You simply specify it in the package configuration file.

**Step 1: Add the library to**  **Cargo.toml** 

toml

\[dependencies]

ariel-os = { version =  "0.1"  } 

ariel_school = { path =  "../ariel_school"  } 



#  Add  any  library  from  Embedded Rust  ecosystem (crates.io) 

\# For example, the popular mathematical library nalgebra-glm

nalgebra-glm = { version =  "0.18"  , default-features =  false  } 



**Step 2. Use it inside the sketch in**  **src/main.rs** 

rust

\#[rustino::sketch]

mod  flix_drone { 

    // Import types from an external library (similar to #include <nalgebra.h>) 

    use  nalgebra_glm::Vec3; 



    let  mut  orientation: Vec3; 



    fn  setup(ctx: &  mut  Context) { 

        // Create a 3D vector (X, Y, Z) from an external library 

        orientation = Vec3::new(  0.0  ,  0.0  ,  0.0  ); 

\}

    

    fn  loop  (ctx: &  mut  Context) { 

        // Logic... 

\}

\}



---

Since  **Rustino**  runs on top of  **Ariel OS**  , under the hood both subsystems use chip-specific asynchronous drivers (Embassy HAL) (  **ESP32-C3**  or  **RP2040**  ). 

---

# PART 9. Peripheral data buses (Serial, SPI)

## 9.1 Serial Port (UART/Serial)

The system distinguishes between two scenarios for using the serial port:

1. **Main debug port (**  **Serial**  **)**  : Initialized by default to the standard USB-CDC or UART0 pins of the board for logging. 

2. **Custom port (**  **ctx.uart**  **)**  : Used to connect external modules. 

**Syntax for working with a custom UART:**

rust

\#[rustino::sketch]

mod  uart_project { 

    let  mut  gps_port: UartDevice; 



    fn  setup(ctx: &  mut  Context) { 

        // Initialize a custom UART on selected pins 

        gps_port = ctx.uart.init( 

ctx.pinout.gpio4,  // TX  pin 

ctx.pinout.gpio5,  // RX  pin 

            9600               //  Bodreit 

\);

\}



    fn  loop  (ctx: &  mut  Context) { 

        // Asynchronous reading of a byte (the function sleeps until the byte arrives) 

        let  byte = gps_port.read_byte(); 

        

        // Sending a byte back 

gps_port.write_byte(byte);

\}

\}



---

## 9.2. SPI Hardware Bus

The SPI bus is critical for connecting fast sensors, displays, or radio modules (such as the NRF24L01 or fast IMUs).

Similar to I2C (  wire  ), the Rustino system provides a secure bus manager object that ensures that  the SCK  ,  MOSI  , and  MISO pins  are initialized exactly once, and that individual devices are separated by the chip select pin (  CS  /  SS  ). 

**Syntax**  **work**  **with**  **SPI:** 

rust

\#[rustino::sketch]

mod  spi_project { 

    let  mut  display: SpiDevice; 



    fn  setup(ctx: &  mut  Context) { 

        // 1. Initialize the common SPI bus (specify SCK, MOSI, MISO) 

        let  spi_bus = ctx.spi.init( 

            ctx.pinout.gpio2,  // SCK (Clock Frequency) 

ctx.pinout.gpio3,  // MOSI (Data Output) 

ctx.pinout.gpio4  // MISO (Data Input) 

\);



        // 2. Connect a specific device by binding it to the CS (Chip Select) pin 

        // and setting the bus frequency (for example, 10 MHz) 

        display = spi_bus.connect_device(ctx.pinout.gpio5,  10_000_000  ); 

\}



    fn  loop  (ctx: &  mut  Context) { 

        // Data buffers for exchange 

        let  tx_data: [u8;  3  ] = [  0x01  ,  0x02  ,  0x03  ]; 

        let  mut  rx_data: [u8;  3  ] = [  0x00  ,  0x00  ,  0x00  ]; 



        // Full-duplex asynchronous data exchange via SPI with automatic control of the CS pin 

        // Under the hood, the macro wraps this in a safe timeout 

        rustino::race! { 

transfer_ok => {

display.transfer(&tx_data, &  mut  rx_data); 

            }, 

spi_timeout => {

delay(  1  );  // Prevent freezing if the device is disconnected 

\}

\}

        

pass();

\}

\}



---

## 9.3 How it works under the hood of macros

When the macro expands  ctx.spi.init(...)  , it checks the validity of the pins for the selected hardware platform: 

- On  **the ESP32-C3**  , it automatically configures the onboard  SPI2 peripherals  and allows pins to be remapped to any GPIO thanks to the I/O MUX. 

- On  **RP2040,**  the compiler strictly checks whether the selected pins belong to the same block (  SPI0  or  SPI1  ), and if the user tries to cross a pin from  SPI0  with a pin from  SPI1  , the project will generate an error at compile time. 

---

# PART 10. Architectural patterns for subsystem implementation

To deploy the ecosystem without errors, a core developer (or high-level AI assistant) needs to rely on the following basic runtime frameworks and procedural macros.

## **10.1.**  Frame  runtime  **(ariel_school/src/lib.rs)** 

This block of code captures the internal structure of the context and proxy context structures, ensuring conflict-free parallel access to the hardware within the Borrow Checker rules.

---

**Code**  **For**  **file**  **ariel_school/src/lib.rs** 

rust

\#![no_std]



\// Re-export the required Ariel OS system components

pub  use  ariel_os; 



\/// Main synchronous context for setup() and loop() functions

pub  struct  Context { 

    pub  pinout: Pinout, 

    pub  i2c: I2cManager, 

    pub  spi: SpiManager, 

    pub  uart: UartManager, 

\}



\/// Thread-safe proxy context for asynchronous workers #[on_event] and #[every]

pub  struct  SubContext { 

    // Internal structures use channels (Channel) or signals (Signal) 

    // from embassy_sync/ariel_os for thread-safe resource allocation. 

\}



pub  struct  Pinout { 

    // Automatically reconfigurable list of available GPIOs for a specific target chip 

\}



pub  struct  I2cManager; 

pub  struct  SpiManager; 

pub  struct  UartManager; 



\/// Global delay function that hides asynchronous .await under the hood

pub  fn  delay(ms: u32) { 

    // Expands to a non-short-circuit asynchronous sleep mode by a macro on Ariel OS 

\}



\/// Function for forced switching of scheduler context

pub  fn  pass() { 

    // Expanded by the macro to yield the processor ariel_os::time::yield_now() 

\}

---

## 10.2. Macro parser framework (ariel_school_macros/src/lib.rs)

A basic parser that processes module structure, extracts let declarations, and moves internal file inclusions (mod name;) outside the Rust compiler sketch.

---

**Code**  **For**  **file**  **ariel_school_macros/src/lib.rs** 

rust

extern  crate  proc_macro; 

use  proc_macro::TokenStream; 

use  quote::quote; 

use  syn::{parse_macro_input, ItemMod, Item}; 



\#[proc_macro_attribute]

pub  fn  sketch(_attr: TokenStream, item: TokenStream) -> TokenStream { 

    let  mut  module = parse_macro_input!(item  as  ItemMod); 

    

    let  mut  extracted_modules = Vec::new(); 

    let  mut  clean_content = Vec::new(); 



    // Parse the contents of the module #[rustino::sketch] 

    if  let  Some((_, items)) = module.content.take() { 

        for  item  in  items { 

            match  item { 

                // If we find a declaration of a submodule (mod name;), we move it outside 

                Item::Mod(m)  if  m.content.is_none() => { 

extracted_modules.push(m);

\}

                //  All  rest  elements  (setup, loop, let mut)  are left  inside 

\_ => clean_content.push(item),

\}

\}

\}



    //  We collect  valid  For  Rust  compiler no_std  structure  code 

    let  expanded = quote! { 

\#![no_std]

        #![no_main] 



        // The externalized modules are now located at the root, and Rust will find their subfiles in src/ 

        #( #extracted_modules ; )* 



        //  Point  entrance  in  Ariel OS 

\#[ariel_os::main]

        async  fn  main() -> !  { 

            // Initializing the hardware HAL chip... 

            // Call the custom setup() and dispatch loop() 

            loop  { 

ariel_os::time::yield_now().  await  ; 

            } 

\}



        // Modified internal user module 

        mod  user_sketch { 

            #( #clean_content )* 

\}

\};



TokenStream::from(expanded)

\}

---

This  **documentation package**  describes: 

1. **Architecture**  (how Rustino sits on top of Ariel OS). 

2. **Syntax of all basic functions**  (GPIO, ADC, I2C, Serial, delays). 

3. **Asynchronous features (**  every  timers  ,  on_event channels  ,  race!/join! macros  ). 

4. **Real code templates**  . 

5. **Instructions for AI neural networks**  (so that they write error-free code). 

6. **Assembler configuration**  (  Cargo.toml  ,  laze-project.yml  ) to support ESP32-C3, RP2040 and RP2040W. 

This set of documents is provided by the  **Rustino project foundation**  . 




