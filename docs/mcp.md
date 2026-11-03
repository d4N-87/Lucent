# Controllo tramite AI (MCP)

L'app include un server [MCP](https://modelcontextprotocol.io) locale: un agente o assistente AI
può leggere e comandare la tastiera con frasi come *"fammi un tramonto con WASD in rosso"*.

## Attivazione

Pulsante **AI** in alto a destra → interruttore. Il server ascolta solo su
`http://127.0.0.1:46327/mcp`: è raggiungibile solo da questo PC, e le richieste con
un `Host` diverso da localhost vengono rifiutate. L'impostazione viene ricordata.

Collegamento da Claude Code: aprendo questa cartella del progetto, il file `.mcp.json` nella
radice lo configura già (Claude Code chiede di approvarlo la prima volta). Altrimenti, da terminale:

```bash
claude mcp add --transport http lucent http://127.0.0.1:46327/mcp
```

Altri client: un server MCP di tipo `http` con lo stesso URL.

## Come funziona

Il server (`src-tauri/src/mcp.rs`) non modifica nulla da solo: inoltra ogni chiamata
all'interfaccia (evento `mcp-call`), che la esegue con le stesse funzioni dell'editor
(`ui/src/App.svelte`, sezione *MCP tools*) e risponde con `mcp_reply`. Quindi le modifiche
dell'AI:

- compaiono dal vivo nell'app e sulla tastiera;
- sono un passo della cronologia (si annullano con Indietro o con lo strumento `undo`);
- finiscono nel profilo attivo, che si salva da solo (colori, tasti e macro);
- per i temi c'è invece il salvataggio esplicito: i colori restano "modificati" rispetto al tema
  finché non si chiama `save_theme`.

L'app deve essere aperta perché gli strumenti funzionino.

## Strumenti

| Strumento | Cosa fa |
|---|---|
| `get_state` | Connessione, profilo attivo, tema da cui vengono i colori (e se sono modificati), sfondo, intensità, tasti personalizzati |
| `list_keys` | Modello, ogni tasto (id, scritta, posizione) e i gruppi |
| `paint_keys` | Colora tasti e/o gruppi (più assegnazioni in una chiamata) |
| `reset_keys` | Riporta tasti al colore di sfondo |
| `set_background` | Colore di sfondo |
| `set_intensity` | Luminosità 0–100 % |
| `apply_gradient` | Sfumatura tra due o più colori, orizzontale o verticale, su tutti o alcuni tasti |
| `apply_harmony` | Palette armonica da un colore base, applicata ai gruppi o come sfumatura |
| `list_profiles` | Profili (colori + tasti + macro): tema, tasti cambiati, macro, quale è attivo |
| `switch_profile` | Attiva un profilo: invia i colori se diversi, scrive tasti e macro solo se diversi da quelli nella tastiera (`keys_written`) |
| `create_profile` | Nuovo profilo, copia di un altro (`copy_of`) o con tasti di fabbrica e i colori attuali; diventa attivo |
| `rename_profile` / `delete_profile` | Gestione dei profili; l'ultimo non si elimina |
| `export_profile` / `import_profile` | Salva un profilo in un file `.lucent-profile.json` (cartella esportazioni) e lo aggiunge da un file, senza attivarlo |
| `list_themes` / `apply_theme` | Temi (i propri e i preimpostati) e applicazione dei colori di un tema al profilo attivo |
| `save_theme` / `revert_to_theme` | Salva i colori nel tema o come nuovo tema; torna ai colori del tema |
| `rename_theme` / `delete_theme` | Gestione dei temi utente; i profili che li usano tengono i colori |
| `undo` / `redo` | Cronologia |
| `send_to_keyboard` | Invio immediato alla tastiera |
| `get_device_info` | Modello, driver, livello di supporto (`verified`, `unverified`, `unconfigured`) e capacità: luminosità ed effetti hardware con i loro parametri |
| `list_models` | Modelli conosciuti (tastiere vere) e formati di disegno (full-size, TKL, 75%, 65%, 60%, ANSI e ISO) |
| `set_preview_format` | Sceglie il formato che disegna una tastiera senza modello; a lei non viene inviato nulla ([nuova-tastiera.md](nuova-tastiera.md)) |
| `get_keyboard_layout` / `set_keyboard_layout` | Formato della tastiera (ISO o ANSI, automatico dal layout di Windows) e layout di Windows che seguono scritte e macro da testo, con i layout installati; non invia nulla alla tastiera |
| `set_report_rate` | Frequenza USB (125/250/500/1000 Hz), salvata nella tastiera; quella attuale è in `get_device_info` |
| `set_effect` | Avvia un effetto hardware (velocità, direzione, colore o colori casuali) |
| `apply_image` | Colora i tasti da un file immagine: ritaglio nelle proporzioni della tastiera, media dei pixel sotto ogni tasto |
| `set_lighting_mode` | Passa tra colori per tasto ed effetto, conservando entrambe le impostazioni |
| `get_key_functions` | Tasti cambiati (altro tasto, multimediale, macro, disattivato), macro con i passaggi, se la tastiera è allineata |
| `set_key_function` | Cambia cosa fa un tasto; scritto subito nella tastiera, annullabile. Fn non si tocca |
| `reset_key_functions` | Riporta tasti (o tutti) alla funzione di fabbrica |
| `save_macro` / `delete_macro` | Crea, sostituisce o elimina una macro salvata nella tastiera (da passaggi o da testo) |
| `get_log` | Ultime righe del registro attività (avvio, tastiera, invii, profili, chiamate MCP) |
| `set_verbose_logging` | Attiva o disattiva il log dettagliato (ogni pacchetto HID in esadecimale) |
| `set_keyboard_look` | Colore di scocca e tasti dell'anteprima (nero, bianco, grigio o esadecimale), per somigliare alla tastiera vera |
| `set_appearance` | Aspetto chiaro, scuro o automatico (segue Windows); quello in uso è `ui_appearance` in `get_state` |
| `list_languages` / `set_language` | Lingue dell'interfaccia trovate in `lang/` e cambio lingua (`auto` segue il sistema) |
| `set_startup_options` | Avvio con Windows (nella tray) e permanenza nella tray alla chiusura; senza argomenti legge lo stato |

Gli strumenti rispondono sempre in inglese. I nomi che vede l'utente (temi, preimpostati,
`ui_name` degli effetti) seguono la lingua dell'interfaccia; `get_state` riporta `ui_language`.
Profili, temi, preimpostati ed effetti si possono indicare con l'id, il nome in inglese o quello mostrato.

## Registro attività

L'app è portatile: profili, impostazioni e log stanno nella cartella `data` accanto
all'eseguibile (se non è scrivibile, ripiega sulle cartelle di sistema `%APPDATA%` e `%LOCALAPPDATA%`).
Il log è `data\logs\lucent.log` (5 file a rotazione da 2 MB). Contiene i percorsi usati all'avvio, connessione e invii alla tastiera
con i tempi, risposte HID scartate, caricamento e salvataggio dei profili, ogni chiamata MCP e gli
errori dell'interfaccia. Il log dettagliato aggiunge ogni pacchetto (`TX`/`RX`). Si attiva da
Impostazioni (icona ingranaggio) → Diagnostica, dove c'è anche il pulsante per aprire la cartella.

Nota per lo sviluppo: l'app desktop di Claude è un pacchetto MSIX, e i programmi avviati dal suo
terminale vedono una copia virtuale di `AppData`. La cartella `data` portatile non ne risente; nel
ripiego su `AppData` conviene avviare l'app con `explorer.exe <percorso dell'exe>`.

I tasti si indicano con id stabili in inglese (`W`, `Esc`, `F1`, `Space`, `ShiftL`, `ShiftR`,
`Num7`, `NumEnter`…), i gruppi con `wasd`, `letters`, `numbers`, `fkeys`, `arrows`, `modifiers`,
`navigation`, `numpad`, `all`. I colori sono stringhe esadecimali `#RRGGBB`.

## Regola per lo sviluppo

Ogni nuova funzione dell'app nasce anche con i suoi strumenti MCP, così l'AI può fare tutto
quello che può fare l'utente.
