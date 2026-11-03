use lucent_core::driver::{Direction, EffectSettings};
use lucent_core::keyboard::{self, ColorBuffer, GmmkDevice};
use lucent_core::protocol::modes::{HardwareEffect, ModeConfig};
use hidapi::HidApi;
use std::io::{self, BufRead, Write};

/// Prints the first 0x40 config bytes (`--read-config`). Read-only, safe to run any time.
pub fn read_config() {
    let api = HidApi::new().expect("HID API");
    let device = match GmmkDevice::open(&api) {
        Ok(dev) => dev,
        Err(e) => return eprintln!("[-] {e}"),
    };
    match device.read_config(0x40) {
        Ok(cfg) => {
            for (i, row) in cfg.chunks(16).enumerate() {
                println!("{:02x}: {:02x?}", i * 16, row);
            }
        }
        Err(e) => eprintln!("[-] {e}"),
    }
}

/// Reads keymap layers (0x07, 3 x 378 bytes) and the macro area (0x09) and saves them as a
/// backup in `dir` (`--read-keys <dir>`). Read-only.
pub fn read_keys(dir: &str) {
    let api = HidApi::new().expect("HID API");
    let device = match GmmkDevice::open(&api) {
        Ok(dev) => dev,
        Err(e) => return eprintln!("[-] {e}"),
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    // The macro area starts with aa 55 and its length (little-endian u16).
    let macro_len = match device.read_block(0x09, 4) {
        Ok(head) if head[0] == 0xAA && head[1] == 0x55 => u16::from_le_bytes([head[2], head[3]]) as usize,
        Ok(head) => return eprintln!("[-] Unexpected macro header: {head:02x?}"),
        Err(e) => return eprintln!("[-] {e}"),
    };
    for (name, cmd, len) in [("layers", 0x07, 3 * 378), ("macros", 0x09, macro_len.max(16))] {
        match device.read_block(cmd, len) {
            Ok(data) => {
                let path = std::path::Path::new(dir).join(format!("{name}-{stamp}.bin"));
                match std::fs::write(&path, &data) {
                    Ok(()) => println!("[+] {name}: {} byte -> {}", data.len(), path.display()),
                    Err(e) => eprintln!("[-] {}: {e}", path.display()),
                }
            }
            Err(e) => eprintln!("[-] {name}: {e}"),
        }
    }
}

/// Writes the 8 mode bytes at config offset 0 (`--write-mode 14 04 03 00 00 ff ff ff`, hex).
pub fn write_mode(hex: &[String]) {
    let data: Result<Vec<u8>, _> = hex.iter().map(|h| u8::from_str_radix(h, 16)).collect();
    let Ok(data) = data.map_err(|e| eprintln!("[-] Byte non valido: {e}")) else { return };
    if data.len() != 8 {
        return eprintln!("[-] Servono 8 byte: effetto luminosità velocità direzione casuale R G B");
    }
    let api = HidApi::new().expect("HID API");
    let device = match GmmkDevice::open(&api) {
        Ok(dev) => dev,
        Err(e) => return eprintln!("[-] {e}"),
    };
    match device.write_config(0x00, &data) {
        Ok(()) => println!("[+] Modalità scritta: {:02x?}", data),
        Err(e) => eprintln!("[-] {e}"),
    }
}

/// Experiment on config byte 0x13 (`--test-13 <00|01> <rrggbb>`): writes the byte, then uploads one
/// color on every key without touching 0x13 again. Unplugging the keyboard afterwards shows whether
/// the colors were kept.
pub fn test_byte_13(args: &[String]) {
    let (Some(value), Some(color)) = (args.first(), args.get(1)) else {
        return eprintln!("[-] Uso: --test-13 <00|01> <rrggbb>");
    };
    let Ok(value) = u8::from_str_radix(value, 16) else { return eprintln!("[-] Valore non valido: {value}") };
    let rgb: Result<Vec<u8>, _> = (0..3).map(|i| u8::from_str_radix(color.get(2 * i..2 * i + 2).unwrap_or("x"), 16)).collect();
    let Ok(rgb) = rgb else { return eprintln!("[-] Colore non valido: {color}") };
    let api = HidApi::new().expect("HID API");
    let device = match GmmkDevice::open(&api) {
        Ok(dev) => dev,
        Err(e) => return eprintln!("[-] {e}"),
    };
    let mut buffer = ColorBuffer::new();
    buffer.set_all([rgb[0], rgb[1], rgb[2]]);
    let result = device
        .write_config(0x13, &[value])
        .and_then(|()| device.upload_colors(&buffer))
        .and_then(|()| device.read_config(0x14));
    match result {
        Ok(cfg) => println!("[+] 0x13 = {:02x}, colori {color} inviati (config 0x13 riletto: {:02x})", value, cfg[0x13]),
        Err(e) => eprintln!("[-] {e}"),
    }
}

/// Lists the connected keyboard's effects (`--effects`). Reads only the device info.
pub fn list_effects() {
    let api = HidApi::new().expect("HID API");
    let kb = match lucent_core::drivers::open_keyboard(&api) {
        Ok(kb) => kb,
        Err(e) => return eprintln!("[-] {e}"),
    };
    println!("{} · {}", kb.info().model, kb.info().driver);
    for fx in &kb.capabilities().effects {
        println!(
            "  {:<14} {:<24} velocità {}  direzioni {:?}  colore {:?}",
            fx.id, fx.name, fx.speed_levels, fx.directions, fx.color
        );
    }
}

/// Runs a hardware effect: `--effect wave speed=2 dir=right color=ff0000|random bright=4`.
pub fn set_effect(args: &[String]) {
    let Some(id) = args.first() else {
        return eprintln!("[-] Uso: --effect <id> [speed=0..3] [dir=left|right] [color=rrggbb|random] [bright=0..4]");
    };
    let mut settings = EffectSettings {
        effect: id.clone(),
        brightness: 4,
        speed: 3,
        direction: None,
        color: Some([0xFF, 0xFF, 0xFF]),
    };
    for arg in &args[1..] {
        let Some((key, value)) = arg.split_once('=') else {
            return eprintln!("[-] Parametro non valido: {arg}");
        };
        let ok = match (key, value) {
            ("speed", v) => v.parse().map(|v| settings.speed = v).is_ok(),
            ("bright", v) => v.parse().map(|v| settings.brightness = v).is_ok(),
            ("dir", "left") => {
                settings.direction = Some(Direction::Left);
                true
            }
            ("dir", "right") => {
                settings.direction = Some(Direction::Right);
                true
            }
            ("color", "random") => {
                settings.color = None;
                true
            }
            ("color", v) => u32::from_str_radix(v, 16)
                .map(|c| settings.color = Some([(c >> 16) as u8, (c >> 8) as u8, c as u8]))
                .is_ok(),
            _ => false,
        };
        if !ok {
            return eprintln!("[-] Parametro non valido: {arg}");
        }
    }
    let api = HidApi::new().expect("HID API");
    let kb = match lucent_core::drivers::open_keyboard(&api) {
        Ok(kb) => kb,
        Err(e) => return eprintln!("[-] {e}"),
    };
    match kb.set_effect(&settings) {
        Ok(()) => println!("[+] {settings:?}"),
        Err(e) => eprintln!("[-] {e}"),
    }
}

/// Console menu, kept for quick hardware tests (`lucent-dev --cli`).
pub fn run() {
    println!("====================================================");
    println!("        Lucent - console di prova (lucent-dev)        ");
    println!("====================================================");

    let api = match HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            eprintln!("Errore inizializzazione HID API: {}", e);
            return;
        }
    };

    let device = match GmmkDevice::open(&api) {
        Ok(dev) => dev,
        Err(e) => {
            eprintln!("[-] Impossibile connettersi alla tastiera: {}", e);
            return;
        }
    };

    let meta = device.metadata();
    println!("[+] Connesso con successo alla tastiera GMMK!");
    println!("    Interfaccia: #{} ({})", meta.interface_number, meta.path);

    let mut current_config = ModeConfig {
        effect: HardwareEffect::Rainbow,
        brightness: 4,
        speed: 3,
        direction: 0,
        random_color: true,
        r: 255,
        g: 255,
        b: 255,
    };

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    loop {
        println!("\n----------------------------------------------------");
        println!("Stato attuale: {} | Luminosità: {}/4", 
            current_config.effect.name(), current_config.brightness);
        println!("----------------------------------------------------");
        println!("--- PROFILI PERSONALIZZATI (ISO CALIBRATO) ---");
        println!("C1. Gamer Cyberpunk (WASD VERI: indici 37, 53, 54, 55 in Rosso)");
        println!("C2. Gradiente Lineare Fluido (Arancione -> Magenta)");
        println!("C4. Spegni tutti i LED (Tastiera Dark)");
        println!("\n--- STRUMENTI DI CALIBRAZIONE ISO ---");
        println!("T.  Test Singolo Indice LED (accende 1 solo tasto alla volta)");
        println!("\n--- EFFETTI HARDWARE PRESET ---");
        println!("1. Rainbow Wave (Onda Arcobaleno)");
        println!("2. Glorious Mode (Modalità Glorious)");
        println!("3. Breathing Rosso");
        println!("5. Statico Bianco (Tutti i tasti)");
        println!("6. Pass Without Trace (Reattivo)");
        println!("L. Cambia Luminosità (0..4)");
        println!("0. Esci");
        print!("\nScegli un'opzione: ");
        io::stdout().flush().unwrap();

        let choice = match lines.next() {
            Some(Ok(s)) => s.trim().trim_start_matches('\u{feff}').to_uppercase(),
            _ => break,
        };

        match choice.as_str() {
            "C1" => {
                println!("[*] Creazione profilo 'Cyberpunk' con indici corretti GMMK...");
                let mut buf = ColorBuffer::new();
                buf.set_all([30, 0, 80]); // Sfondo viola scuro

                // Slot dalla keymap della tastiera: W = 37, A = 53, S = 54, D = 55
                buf.set_wasd([255, 0, 0]); // Rosso fuoco
                buf.set_key(keyboard::layout::KEY_ESC, [255, 120, 0]); // Esc arancione
                buf.set_key(keyboard::layout::KEY_SPACE, [0, 255, 255]); // Barra spaziatrice ciano

                current_config.effect = HardwareEffect::Custom;
                match device.set_custom_colors(&buf, current_config.brightness) {
                    Ok(_) => println!("[+] Profilo Cyberpunk inviato! Guarda W-A-S-D ora!"),
                    Err(e) => eprintln!("[-] Errore: {}", e),
                }
                continue;
            }
            "C2" => {
                println!("[*] Invio gradiente lineare...");
                let mut buf = ColorBuffer::new();
                buf.apply_linear_gradient([255, 60, 0], [220, 0, 180]);

                current_config.effect = HardwareEffect::Custom;
                match device.set_custom_colors(&buf, current_config.brightness) {
                    Ok(_) => println!("[+] Gradiente applicato!"),
                    Err(e) => eprintln!("[-] Errore: {}", e),
                }
                continue;
            }
            "C4" => {
                println!("[*] Spegnimento tutti i LED...");
                let buf = ColorBuffer::new();

                current_config.effect = HardwareEffect::Custom;
                match device.set_custom_colors(&buf, 0) {
                    Ok(_) => println!("[+] Tutti i LED spenti!"),
                    Err(e) => eprintln!("[-] Errore: {}", e),
                }
                continue;
            }
            "T" => {
                print!("Inserisci il numero dell'indice LED da testare (0..143): ");
                io::stdout().flush().unwrap();
                if let Some(Ok(val)) = lines.next() {
                    if let Ok(idx) = val.trim().parse::<usize>() {
                        let mut buf = ColorBuffer::new();
                        buf.set_key(idx, [255, 0, 0]); // Solo questo tasto in rosso
                        match device.set_custom_colors(&buf, 4) {
                            Ok(_) => println!("[+] Acceso SOLO il LED indice {}. Quale tasto si è illuminato?", idx),
                            Err(e) => eprintln!("[-] Errore: {}", e),
                        }
                    } else {
                        println!("Numero non valido.");
                    }
                }
                continue;
            }
            "1" => {
                current_config.effect = HardwareEffect::Rainbow;
                current_config.random_color = true;
            }
            "2" => {
                current_config.effect = HardwareEffect::GloriousMode;
                current_config.random_color = true;
            }
            "3" => {
                current_config.effect = HardwareEffect::Breathing;
                current_config.random_color = false;
                current_config.r = 255;
                current_config.g = 0;
                current_config.b = 0;
            }
            "5" => {
                current_config.effect = HardwareEffect::NormallyOn;
                current_config.random_color = false;
                current_config.r = 255;
                current_config.g = 255;
                current_config.b = 255;
            }
            "6" => {
                current_config.effect = HardwareEffect::PassWithoutTrace;
                current_config.random_color = true;
            }
            "L" => {
                print!("Inserisci luminosità (0 = Spento, 4 = Max): ");
                io::stdout().flush().unwrap();
                if let Some(Ok(val)) = lines.next()
                    && let Ok(b) = val.trim().parse::<u8>()
                {
                    current_config.brightness = b.min(4);
                }
            }
            "0" | "Q" | "EXIT" => {
                println!("Uscita dal programma. Ciao!");
                break;
            }
            _ => {
                println!("Opzione non valida, riprova.");
                continue;
            }
        }

        println!("[*] Invio configurazione alla tastiera in corso...");
        match device.set_hardware_mode(&current_config) {
            Ok(_) => println!("[+] Fatto! Effetto '{}' applicato.", current_config.effect.name()),
            Err(e) => eprintln!("[-] Errore: {}", e),
        }
    }
}
