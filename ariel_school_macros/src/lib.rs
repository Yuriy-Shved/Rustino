extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, File, Item};

/// Макрос для одного рабочего цикла (Аналог классической Arduino)
#[proc_macro_attribute]
pub fn school_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_file = parse_macro_input!(item as File);
    
    let mut setup_fn: Option<ItemFn> = None;
    let mut loop_fn: Option<ItemFn> = None;
    let mut other_items = Vec::new();

    // Сортируем элементы файла: отделяем setup и loop_tick от глобальных переменных/функций
    for item in input_file.items {
        match item {
            Item::Fn(f) => {
                if f.sig.ident == "setup" {
                    setup_fn = Some(f);
                } else if f.sig.ident == "loop_tick" {
                    loop_fn = Some(f);
                } else {
                    other_items.push(Item::Fn(f));
                }
            }
            _ => other_items.push(item),
        }
    }

    let setup_body = setup_fn.map(|f| f.block).unwrap_or_else(|| syn::parse_quote!({}));
    let loop_body = match loop_fn {
        Some(f) => f.block,
        None => panic!("Ошибка компиляции: Вы забыли написать обязательную функцию fn loop_tick()!"),
    };

    // Генерируем скрытую асинхронную инфраструктуру Ariel OS
    let expanded = quote! {
        #(#other_items)*

        #[ariel_os::main]
        async fn main() {
            // Скрытая инициализация HAL-слоя из Этапа 1
            ariel_school::internal::init();

            // Выполнение школьного блока настроек setup
            {
                #setup_body
            }

            // Главный цикл операционной системы
            loop {
                // Исполнение школьного кода loop_tick
                {
                    #loop_body
                }
                
                // Микро-пауза для предотвращения зависания планировщика (минимальный такт),
                // если школьник не поставил функцию delay() в свой код.
                ariel_os::time::Timer::after(ariel_os::time::Duration::from_millis(1)).await;
            }
        }
    };

    TokenStream::from(expanded)
}

/// Продвинутый макрос для автоматического распределения задач по двум ядрам RP2040
#[proc_macro_attribute]
pub fn school_dual_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_file = parse_macro_input!(item as File);
    
    let mut setup_fn: Option<ItemFn> = None;
    let mut loop1_fn: Option<ItemFn> = None;
    let mut loop2_fn: Option<ItemFn> = None;
    let mut other_items = Vec::new();

    for item in input_file.items {
        if let Item::Fn(f) = item {
            if f.sig.ident == "setup" { setup_fn = Some(f); }
            else if f.sig.ident == "loop_tick_1" { loop1_fn = Some(f); }
            else if f.sig.ident == "loop_tick_2" { loop2_fn = Some(f); }
            else { other_items.push(Item::Fn(f)); }
        } else {
            other_items.push(item);
        }
    }

    let setup_body = setup_fn.map(|f| f.block).unwrap_or_else(|| syn::parse_quote!({}));
    let loop1_body = loop1_fn.map(|f| f.block).expect("Ошибка: Отсутствует функция fn loop_tick_1() для Ядра 0");
    let loop2_body = loop2_fn.map(|f| f.block).expect("Ошибка: Отсутствует функция fn loop_tick_2() для Ядра 1");

    let expanded = quote! {
        #(#other_items)*

        // Аппаратное резервирование изолированных участков RAM под стеки независимых потоков [A, 5]
        static mut STACK_CORE_0: [u8; 1024] = [0; 1024];
        static mut STACK_CORE_1: [u8; 1024] = [0; 1024];

        #[ariel_os::main]
        async fn main() {
            ariel_school::internal::init();
            
            { #setup_body }

            // Поток 1: Жестко привязывается ИИ-макросом к Core 0 (affinity = 1)
            ariel_os::thread::Builder::new()
                .affinity(1)
                .spawn(unsafe { &mut STACK_CORE_0 }, || {
                    loop {
                        { #loop1_body }
                        ariel_os::time::blocking_delay(ariel_os::time::Duration::from_millis(1));
                    }
                }).expect("Не удалось запустить поток на Core 0");

            // Поток 2: Жестко привязывается ИИ-макросом к Core 1 (affinity = 2)
            ariel_os::thread::Builder::new()
                .affinity(2)
                .spawn(unsafe { &mut STACK_CORE_1 }, || {
                    loop {
                        { #loop2_body }
                        ariel_os::time::blocking_delay(ariel_os::time::Duration::from_millis(1));
                    }
                }).expect("Не удалось запустить поток на Core 1");
            
            // Основной поток main засыпает в режиме WFI, освобождая вычислительные ресурсы ядрам
            loop {
                ariel_os::time::asm::wfi(); 
            }
        }
    };

    TokenStream::from(expanded)
}
