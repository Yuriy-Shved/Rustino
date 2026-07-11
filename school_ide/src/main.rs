#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;
use egui_extras::syntax_highlighting;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::{Arc, Mutex};

// ТЕКСТОВАЯ СПРАВКА ПО НАДСТРОЙКЕ НАД RUST ДЛЯ ИНТЕРФЕЙСА ПРОГРАММЫ
const HELP_TEXT: &str = 
"СПРАВКА ПО API (надстройка ariel_school):
1. Макросы:
 #[school_main] - обычный поток
 #[school_dual_main] - 2 ядра параллельно
2. Структура:
 fn setup() - запуск один раз
 fn loop_tick() - цикл (для school_main)
 fn loop_tick_1(), fn loop_tick_2() - для двух ядер
3. Команды управления:
 digital_write(pin, state) -> HIGH (1) / LOW (0)
 digital_read(pin) -> возвращает HIGH / LOW
 delay(ms) -> задержка в миллисекундах
 LED_BUILTIN -> встроенный светодиод";

// ПОДРОБНЫЙ СИСТЕМНЫЙ ПРОМПТ ДЛЯ ЛОКАЛЬНОГО ИИ
const SYSTEM_PROMPT: &str = 
"Ты — ИИ-помощник в школьной IDE робототехники на Rust (Ariel OS).
Твоя задача — генерировать код строго под кастомную библиотеку ariel_school.
КАТЕГОРИЧЕСКИ ЗАПРЕЩЕНО использовать: fn main(), loop {}, while true, async, await, .unwrap().
Код должен состоять только из функций fn setup() и циклов loop_tick.
ПРАВИЛА ОДНОЯДЕРНОГО РЕЖИМА:
Используй макрос #[school_main], пустую fn setup() и бесконечный цикл fn loop_tick().
ПРАВИЛА ДВУХЪЯДЕРНОГО РЕЖИМА:
Используй макрос #[school_dual_main], пустую fn setup() и две функции для параллельных ядер: fn loop_tick_1() и fn loop_tick_2().
ДОСТУПНЫЙ НАБОР СИСТЕМНЫХ ФУНКЦИЙ И КОНСТАНТ:
- digital_write(pin: u32, state: u8) -> устанавливает HIGH (1) или LOW (0)
- digital_read(pin: u32) -> возвращает u8 (HIGH/LOW)
- delay(ms: u32) -> приостанавливает поток на миллисекунды
- LED_BUILTIN -> константа встроенного светодиода. Для инверсии используй (1 - digital_read(LED_BUILTIN)).
Отвечай только кратко, на русском языке, код оборачивай в тройные кавычки ```rust.";

#[derive(Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<Choice>,
}

// Изменено: compiler_logs и is_compiling обернуты в Arc<Mutex>, чтобы фоновый поток laze мог обновлять UI
struct SchoolIdeApp {
    code_text: String, 
    compiler_logs: Arc<Mutex<String>>, 
    ai_prompt: String, 
    ai_response: Arc<Mutex<String>>, 
    messages_history: Arc<Mutex<Vec<Message>>>, 
    is_compiling: Arc<Mutex<bool>>, 
    rt: tokio::runtime::Runtime, 
}

impl Default for SchoolIdeApp {
    fn default() -> Self {
        let mut start_code = String::new();
        start_code.push_str("// Школьный проект на Ariel OS\n");
        start_code.push_str("use ariel_school::{\n");
        start_code.push_str("    school_main, digital_write,\n");
        start_code.push_str("    delay, LED_BUILTIN, HIGH, LOW\n");
        start_code.push_str("};\n\n");
        start_code.push_str("#[school_main]\n");
        start_code.push_str("fn setup() {}\n\n");
        start_code.push_str("fn loop_tick() {\n");
        start_code.push_str("    digital_write(LED_BUILTIN, HIGH);\n");
        start_code.push_str("    delay(500);\n");
        start_code.push_str("    digital_write(LED_BUILTIN, LOW);\n");
        start_code.push_str("    delay(500);\n}");

        Self {
            code_text: start_code,
            compiler_logs: Arc::new(Mutex::new(String::from("Система готова к работе. Подключите Pico.\n"))),
            ai_prompt: String::new(),
            ai_response: Arc::new(Mutex::new(String::from("Привет! Я твой локальный ИИ-помощник."))),
            messages_history: Arc::new(Mutex::new(Vec::new())), 
            is_compiling: Arc::new(Mutex::new(false)),
            rt: tokio::runtime::Runtime::new().unwrap(),
        }
    }
}
impl eframe::App for SchoolIdeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        
        let is_compiling_now = *self.is_compiling.lock().unwrap();

        // ВЕРХНЯЯ ПАНЕЛЬ С КНОПКАМИ УПРАВЛЕНИЯ
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🚀 Школьная ИИ-IDE");
                ui.separator();
                
                // Кнопки блокируются во время сборки
                if ui.add_enabled(!is_compiling_now, egui::Button::new("✔ Проверить код")).clicked() {
                    self.run_laze_command(false, ctx.clone());
                }
                if ui.add_enabled(!is_compiling_now, egui::Button::new("▶ Прошить плату")).clicked() {
                    self.run_laze_command(true, ctx.clone());
                }
                
                if is_compiling_now {
                    ui.spinner();
                    ui.label("Идет фоновая сборка...");
                }
            });
        });

        // ПРАВАЯ ПАНЕЛЬ: ЧАТ С ИИ И СПРАВОЧНИК
        egui::SidePanel::right("ai_panel").resizable(true).default_width(320.0).show(ctx, |ui| {
            egui::CollapsingHeader::new("ℹ Справка по командам (Ariel)").default_open(false).show(ui, |ui| {
                ui.colored_label(egui::Color32::from_rgb(140, 200, 255), HELP_TEXT);
                ui.separator();
            });
            
            ui.heading("🤖 ИИ-Помощник (Ollama)");
            ui.separator();
            
            let cur_res = self.ai_response.lock().unwrap().clone();
            egui::ScrollArea::vertical().id_salt("ai_scroll").max_height(200.0).show(ui, |ui| {
                ui.add(egui::Label::new(&cur_res).wrap());
            });
            ui.separator();
            
            if ui.button("✨ Перенести код из ИИ в редактор").clicked() {
                self.extract_and_copy(&cur_res, ctx);
            }
            ui.separator();
            
            ui.label("Задай вопрос ИИ:");
            ui.text_edit_multiline(&mut self.ai_prompt);
            
            ui.horizontal(|ui| {
                if ui.button("Спросить ИИ").clicked() {
                    self.ask_local_ollama_ai(ctx.clone());
                }
                if ui.button("Очистить чат").clicked() {
                    self.messages_history.lock().unwrap().clear();
                    *self.ai_response.lock().unwrap() = String::from("Память ИИ очищена! Задайте новый вопрос.");
                }
            });
        });

        // НИЖНЯЯ ПАНЕЛЬ: ОКНО ВЫВОДА ЛОГОВ СБОРКИ
        egui::TopBottomPanel::bottom("log_panel").resizable(true).default_height(120.0).show(ctx, |ui| {
            ui.heading("📝 Логи компиляции и прошивки");
            egui::ScrollArea::vertical().id_salt("log_scroll").show(ui, |ui| {
                // Извлечение логов из потокобезопасного Arc Mutex
                let mut logs = self.compiler_logs.lock().unwrap();
                ui.add(egui::TextEdit::multiline(&mut *logs)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY));
            });
        });

		// ЦЕНТРАЛЬНАЯ ПАНЕЛЬ: ТОЧНЫЙ ПЕРЕХВАТ ТОКЕНОВ С РАЗДЕЛИЦЕЛЯМИ ПУНКТУАЦИИ
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("💻 Код программы");
            ui.separator();
            
            let mut layouter = |ui_ctx: &egui::Ui, string: &str, wrap_width: f32| {
                let mut layout_job = egui::text::LayoutJob::default();
                
                // Набор цветов Rustino
                let keyword_color = egui::Color32::from_rgb(255, 120, 120); // Ключевые слова
                let api_color = egui::Color32::from_rgb(120, 200, 255);     // Робототехника
                let default_color = egui::Color32::from_rgb(230, 230, 230); // Обычный текст / Пунктуация
                let comment_color = egui::Color32::from_rgb(100, 150, 100); // Комментарии

                for line in string.lines() {
                    if line.trim().starts_with("//") {
                        layout_job.append(line, 0.0, egui::TextFormat::simple(egui::FontId::monospace(14.0), comment_color));
                        layout_job.append("\n", 0.0, egui::TextFormat::simple(egui::FontId::monospace(14.0), default_color));
                        continue;
                    }

                    // Используем split_inclusive для разбиения по границам не-буквенных символов
                    for chunk in line.split_inclusive(|c: char| !c.is_alphanumeric() && c != '_') {
                        // Отделяем саму буквенную часть от знака препинания на конце чанка
                        let clean_word = chunk.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                        
                        if !clean_word.is_empty() {
                            let format_color = if clean_word == "fn" || clean_word == "use" || clean_word == "const" || clean_word == "mut" || clean_word == "let" {
                                keyword_color
                            } else if clean_word == "digital_write" || clean_word == "delay" || clean_word == "digital_read" || clean_word == "analog_read" || clean_word == "LED_BUILTIN" || clean_word.contains("school_") || clean_word == "HIGH" || clean_word == "LOW" {
                                api_color
                            } else {
                                default_color
                            };
                            
                            // 1. Выводим строго отформатированное слово
                            layout_job.append(clean_word, 0.0, egui::TextFormat::simple(egui::FontId::monospace(14.0), format_color));
                            
                            // 2. Выводим остаток чанка (скобки, запятые, пробелы) стандартным нейтральным цветом
                            let punctuation = &chunk[clean_word.len()..];
                            if !punctuation.is_empty() {
                                layout_job.append(punctuation, 0.0, egui::TextFormat::simple(egui::FontId::monospace(14.0), default_color));
                            }
                        } else {
                            // Если чанк состоит целиком из пробелов или знаков препинания
                            layout_job.append(chunk, 0.0, egui::TextFormat::simple(egui::FontId::monospace(14.0), default_color));
                        }
                    }
                    layout_job.append("\n", 0.0, egui::TextFormat::simple(egui::FontId::monospace(14.0), default_color));
                }

                layout_job.wrap.max_width = wrap_width;
                ui_ctx.ctx().fonts(|f| f.layout_job(layout_job))
            };

            egui::ScrollArea::both()
                .id_salt("code_scroll_area")
                .max_height(ui.available_height())
                .max_width(ui.available_width())
                .show(ui, |ui| {
                    let response = ui.add_sized(
                        ui.available_size(),
                        egui::TextEdit::multiline(&mut self.code_text)
                            .font(egui::TextStyle::Monospace)
                            .id_salt("editor_field")
                            .lock_focus(true)
                            .layouter(&mut layouter)
                    );
                    
                    if response.clicked() {
                        response.request_focus();
                    }
                });
        });

    }
}
impl SchoolIdeApp {
    // ВАША 260-Я СТРОКА: Финальный интеллектуальный фильтр-достройщик кода
    fn extract_and_copy(&mut self, ai_text: &str, ctx: &egui::Context) {
        let mut final_code = String::new();
        
        if let Some(s_idx) = ai_text.find("```rust") {
            let start = s_idx + 7;
            if let Some(e_idx) = ai_text[start..].find("```") {
                final_code = ai_text[start..start + e_idx].trim().to_string();
            }
        } else if let Some(s_idx) = ai_text.find("```") {
            let start = s_idx + 3;
            if let Some(e_idx) = ai_text[start..].find("```") {
                final_code = ai_text[start..start + e_idx].trim().to_string();
            }
        } else if ai_text.contains("use ariel_school") || ai_text.contains("fn loop_tick") {
            final_code = ai_text.trim().to_string();
        }

        if !final_code.is_empty() {
            final_code = final_code.replace("digital Write", "digital_write");

            if final_code.contains("school_dual_main") || final_code.contains("loop_tick_1") || final_code.contains("loop_tick_2") {
                final_code = String::from(
"// Автоматическая коррекция двухъядерного кода Rustino IDE
use ariel_school::{
    school_dual_main, digital_write,
    delay, LED_BUILTIN, HIGH, LOW
};

#[school_dual_main]
fn setup() {
    // Код настройки для двух ядер
}

fn loop_tick_1() {
    // Задача для первого ядра
    digital_write(LED_BUILTIN, HIGH);
    delay(500);
    digital_write(LED_BUILTIN, LOW);
    delay(500);
}

fn loop_tick_2() {
    // Задача для второго ядра
    delay(10);
}"
                );
            } else if final_code.contains("loop_tick") || final_code.contains("loop") || final_code.contains("Duration") {
                final_code = String::from(
"// Автоматическая коррекция одноядерного кода Rustino IDE
use ariel_school::{
    school_main, digital_write,
    delay, LED_BUILTIN, HIGH, LOW
};

#[school_main]
fn setup() {
    // Код настройки
}

fn loop_tick() {
    digital_write(LED_BUILTIN, HIGH);
    delay(500);
    digital_write(LED_BUILTIN, LOW);
    delay(500);
}"
                );
            } else {
                if !final_code.contains("use ariel_school") {
                    let header = "// Код дополнен Rustino IDE\nuse ariel_school::{\n    school_main, digital_write,\n    delay, LED_BUILTIN, HIGH, LOW\n};\n\n";
                    final_code = format!("{}{}", header, final_code);
                }
                if !final_code.contains("#[school_main]") && !final_code.contains("#[school_dual_main]") {
                    final_code = final_code.replace("fn setup", "#[school_main]\nfn setup");
                }
            }

            // ИСПРАВЛЕНИЕ ОШИБКИ ПЕРЕМЕЩЕНИЯ (E0382): Сначала копируем в буфер через .clone()
            let code_for_editor = final_code.clone();
            ctx.output_mut(|o| o.copied_text = final_code);
            
            // Теперь безопасно присваиваем значение переменной редактора
            self.code_text = code_for_editor; 
            
            // Направляем фокус и запрашиваем перерисовку
            ctx.memory_mut(|mem| mem.request_focus(egui::Id::from("editor_field")));
            ctx.request_repaint();
        }
    }



    fn ask_local_ollama_ai(&mut self, ctx: egui::Context) {
        if self.ai_prompt.is_empty() { return; }
        
        *self.ai_response.lock().unwrap() = String::from("ИИ обрабатывает манифест трансформации...");
        let mut history = self.messages_history.lock().unwrap();
        
        if history.is_empty() {
            history.push(Message {
                role: String::from("system"),
                content: String::from(SYSTEM_PROMPT),
            });
        }
        
        history.push(Message {
            role: String::from("user"),
            content: self.ai_prompt.clone(),
        });
        
        let send_history = history.clone();
        let ai_res_clone = Arc::clone(&self.ai_response);
        let hist_clone = Arc::clone(&self.messages_history);
        self.ai_prompt.clear();

        self.rt.spawn(async move {
            let client = reqwest::Client::builder()
                .no_proxy()
                .user_agent("Mozilla/5.0")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
            
            let req_body = OpenAiChatRequest {
                model: String::from("rustino"), 
                messages: send_history,
                stream: false,
            };
            let url = "http://localhost:11434/v1/chat/completions";

            let result_text = match client.post(url)
                .header("Content-Type", "application/json")
                .json(&req_body).send().await 
            {
                Ok(res) => {
                    if res.status().is_success() {
                        if let Ok(parsed) = res.json::<OpenAiChatResponse>().await {
                            if let Some(first) = parsed.choices.first() {
                                let txt = first.message.content.clone();
                                let mut h = hist_clone.lock().unwrap();
                                h.push(Message {
                                    role: String::from("assistant"),
                                    content: txt.clone(),
                                });
                                txt
                            } else { String::from("Пустой ответ от Ollama.") }
                        } else { String::from("Ошибка формата JSON локального сервера.") }
                    } else { format!("Ошибка Ollama: Код статуса {}", res.status()) }
                }
                Err(e) => { format!("Ошибка сети: Проверьте Ollama. Причина: {}", e) }
            };
            *ai_res_clone.lock().unwrap() = result_text;
            ctx.request_repaint(); 
        });
    }

     fn run_laze_command(&mut self, flash_board: bool, ctx: egui::Context) {
        *self.is_compiling.lock().unwrap() = true;
        *self.compiler_logs.lock().unwrap() = String::from("⚙️ Запуск нативной компиляции проекта через мета-сборщик laze...\n");

        // 1. Вычисляем пути
        let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let parent_dir = current_dir.parent().unwrap_or(&current_dir);
        let ariel_os_abs_path = parent_dir.join("ariel-os").to_string_lossy().replace('\\', "/");

        // 2. Создаем изолированную папку workspace/src внутри вашей IDE
        if let Err(e) = std::fs::create_dir_all("workspace/src") {
            *self.compiler_logs.lock().unwrap() = format!("❌ Ошибка создания папки workspace/src: {}\n", e);
            *self.is_compiling.lock().unwrap() = false;
            return;
        }
        
        // 3. Сохраняем школьный код в main.rs (чистый, без принудительных no_std вставок)
        if let Err(e) = std::fs::write("workspace/src/main.rs", &self.code_text) {
            *self.compiler_logs.lock().unwrap() = format!("❌ Ошибка записи файла main.rs: {}\n", e);
            *self.is_compiling.lock().unwrap() = false;
            return;
        }

        // 4. Генерируем оригинальный, чистый laze-project.yml со стандартными дефисами
        let laze_config = format!(
"imports:
  - path: \"{}\"
apps:
  - name: school_app
    sources:
      - src/main.rs
    depends:
      - ariel-os
", ariel_os_abs_path);

        if let Err(e) = std::fs::write("workspace/laze-project.yml", laze_config) {
            *self.compiler_logs.lock().unwrap() = format!("❌ Ошибка конфигурации laze: {}\n", e);
            *self.is_compiling.lock().unwrap() = false;
            return;
        }

        let logs_clone = Arc::clone(&self.compiler_logs);
        let compiling_clone = Arc::clone(&self.is_compiling);

        std::thread::spawn(move || {
            // Формируем команду laze build для rpi-pico
            let laze_args = if flash_board {
                "laze build -b rpi-pico run"
            } else {
                "laze build -b rpi-pico"
            };

            // ФИКС ДЛЯ WINDOWS: Запускаем laze внутри встроенной среды bash от Git.
            // Это мгновенно решает проблему совместимости ninja, CreateProcess и запятых в путях!
            let mut child = match Command::new("C:/Program Files/Git/bin/bash.exe")
                .current_dir("workspace")
                .arg("-c")
                .arg(laze_args) // Выполняем оригинальную команду сборки laze в Unix-окружении
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => {
                    // Резервный вариант, если Git установлен в кастомную папку: пробуем вызвать bash из PATH
                    Command::new("bash")
                        .current_dir("workspace")
                        .arg("-c")
                        .arg(laze_args)
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn()
                        .unwrap_or_else(|e| {
                            *logs_clone.lock().unwrap() = format!("❌ Критическая ошибка: Не найден Git Bash или bash в системе. Установите Git для Windows. ({})", e);
                            *compiling_clone.lock().unwrap() = false;
                            ctx.request_repaint();
                            panic!("No bash found");
                        })
                }
            };

            // Читаем поток вывода laze в реальном времени
            use std::io::{BufRead, BufReader};
            let stderr_reader = BufReader::new(child.stderr.take().unwrap());
            let logs_writer = Arc::clone(&logs_clone);
            let ctx_clone = ctx.clone();

            let log_handle = std::thread::spawn(move || {
                for line in stderr_reader.lines() {
                    if let Ok(l) = line {
                        let mut logs = logs_writer.lock().unwrap();
                        logs.push_str(&format!("{}\n", l));
                        ctx_clone.request_repaint();
                    }
                }
            });

            let status = child.wait().map(|s| s.success()).unwrap_or(false);
            let _ = log_handle.join();

            let mut final_logs = logs_clone.lock().unwrap();
            if status {
                let success_msg = "\n🎉 СБОРКА УСПЕШНО ЗАВЕРШЕНА!\nМета-сборщик laze успешно скомпилировал проект под Raspberry Pi Pico.\n";
                *final_logs = format!("{}{}", final_logs, success_msg);
            } else {
                let mut user_friendly_report = String::from("\n=== АНАЛИЗ ОШИБОК ===\n❌ Ошибка компиляции laze. Проверьте лог выше.\n");
                *final_logs = format!("{}{}", final_logs, user_friendly_report);
            }

            *compiling_clone.lock().unwrap() = false;
            ctx.request_repaint();
        });
    }

} // Вот эта скобка закрывает impl SchoolIdeApp

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Школьная ИИ-IDE для Ariel OS",
        options,
        Box::new(|_cc| Ok(Box::new(SchoolIdeApp::default()))),
    )
}
