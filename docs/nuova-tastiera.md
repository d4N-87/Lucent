# Aggiungere una tastiera

L'app conosce una tastiera tramite un **file del modello**: dice come riconoscerla su USB, quale
driver usare e dove sta ogni tasto. Oggi c'è un solo modello verificato, la GMMK v1 full-size ISO
italiana (`crates/lucent-core/models/gmmk_v1_fullsize_iso_it.json`).

## Cosa fa l'app con una tastiera che non conosce

| Situazione | Cosa vede l'utente | Cosa invia l'app alla tastiera |
|---|---|---|
| Modello incluso e verificato | niente di speciale | tutto |
| Modello da un file nella cartella `models` | avviso "Modello non verificato" | colori sì; tasti e macro solo dopo il permesso dell'utente nella sezione Tasti |
| Tastiera di una famiglia nota (Sonix `0C45`, Glorious `320F`) senza modello | avviso "Tastiera non configurata", con VID e PID | **niente**, neanche comandi di lettura: si vede solo un'anteprima con un formato scelto |
| Altre tastiere | "Nessuna tastiera trovata" | niente |

Con una tastiera non configurata colori e profili non vengono salvati: l'anteprima usa un formato
di disegno (`crates/lucent-core/formats/`: full-size, TKL, 75%, 65%, 60%, ANSI e ISO) i cui tasti
non corrispondono a quelli dei profili.

## 1. Capire se il protocollo è quello della GMMK v1

Il driver `gmmk_v1` parla il protocollo descritto in [protocollo.md](protocollo.md): pacchetti da
64 byte sull'interfaccia vendor (usage page `0xFF1C`), colori con il comando `0x11`, tasti con
`0x08`, macro con `0x0A`. Altre tastiere con chip Sonix e firmware originale spesso lo usano
uguale o quasi. Per saperlo:

1. installa [Wireshark](https://www.wireshark.org/) con USBPcap e avvialo come amministratore;
2. cattura mentre l'app ufficiale cambia il colore di **un solo tasto**;
3. confronta i pacchetti con quelli descritti in `protocollo.md`.

Se sono diversi serve un driver nuovo (codice Rust in `crates/lucent-core/src/drivers/`), non solo
un file.

## 2. Scrivere il file del modello

Parti da un formato simile in `crates/lucent-core/formats/` o dal modello della GMMK v1.

| Campo | Esempio | Significato |
|---|---|---|
| `id` | `"gmmk_v1_tkl_ansi_us"` | Nome unico, senza spazi |
| `name` | `"GMMK TKL"` | Nome mostrato |
| `format` / `standard` / `legends` | `"tkl"` / `"ansi"` / `"us"` | Formato, ANSI o ISO, lingua delle scritte |
| `usb` | `{"vid": "0C45", "pid": "652F"}` | Codici USB in esadecimale (l'avviso dell'app li mostra) |
| `driver` | `"gmmk_v1"` | Driver che parla con la tastiera |
| `leds` | `144` | Dimensione del buffer dei colori (per la GMMK v1 sempre 144) |
| `width` / `height` | `18.25` / `6.5` | Ingombro in unità tasto |
| `keys` | vedi sotto | Un elemento per tasto |

Ogni tasto: `{"slot": 37, "id": "W", "label": "W", "x": 2.5, "y": 2.5, "w": 1, "h": 1}`.

- `slot` è la posizione del tasto nella mappa dei tasti e nel buffer dei colori (sulla GMMK v1
  coincidono). È il dato difficile: colora un tasto alla volta nell'app ufficiale e guarda quale
  terna di byte cambia nel comando `0x11` (posizione ÷ 3 = slot).
- `id` è il nome stabile usato da profili e strumenti MCP: usa gli stessi della GMMK v1
  (`ShiftL`, `NonUsBackslash`, `Num7`…), così gruppi e preimpostati funzionano subito.
- `x`, `y`, `w`, `h` sono in unità tasto dall'angolo in alto a sinistra.
- `label` è la scritta di riserva. L'app la sostituisce: i tasti che scrivono un carattere
  (lettere, cifre, simboli) mostrano quello del layout di Windows in uso, e i tasti con un nome
  (Invio, Shift, BlocScorr…) quello della sezione `[keycap]` dei file lingua.

Di un modello ISO l'app ricava da sola anche la forma ANSI (Impostazioni › Tastiera › Formato):
toglie `NonUsBackslash`, allarga `ShiftL`, rende l'Invio basso e trasforma `NonUsHash` nel
`Backslash` sopra di lui, con gli stessi slot. Finché non è provata su una tastiera ANSI vera, in
quella forma il modello resta "non verificato" (tasti e macro chiedono il permesso). Se una
tastiera ANSI usa slot diversi per quei tasti, serve un suo file del modello con `"standard": "ansi"`.

Per i modelli diversi da quello incluso, la mappa di fabbrica dei tasti viene letta dalla tastiera
(comando `0x0F`), non dal file.

## 3. Provarlo

1. Copia il file nella cartella `models` accanto all'eseguibile (crea la cartella se non c'è).
2. Avvia l'app: il log dice se il file è stato caricato o perché è stato scartato.
3. La tastiera compare come "Modello non verificato". Prova i colori tasto per tasto, poi, se vuoi,
   consenti la scrittura dei tasti: prima della prima scrittura l'app salva un backup della mappa e
   delle macro in `data/backup`.

Per provare gli avvisi senza avere un'altra tastiera, avvia l'app con la variabile d'ambiente
`LUCENT_SIMULATE_UNKNOWN=1`: finge una tastiera Glorious senza modello e non invia nulla.

## 4. Proporlo per tutti

Quando colori, tasti e macro funzionano, aggiungi il file in `crates/lucent-core/models/`, elencalo in
`BUILTIN_MODELS` (`crates/lucent-core/src/keyboard/model.rs`), metti `"verified": true` e apri una
pull request con quello che hai provato. I test controllano che slot e id non si ripetano.
