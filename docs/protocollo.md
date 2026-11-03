# Protocollo GMMK (firmware originale)

Ricavato dalle catture USB dell'app Glorious (`captures/`, non versionate).
Tastiera: GMMK v1 full-size ISO, VID `0C45`, PID `652F`, controller Sonix SN32F248B.

## Trasporto

- Interfaccia 1 (`MI_01`), collection vendor (`COL04`).
- Interrupt OUT su endpoint `0x03`, risposta interrupt IN su `0x82`.
- Ogni pacchetto è lungo 64 byte. La tastiera risponde a ogni comando con un pacchetto: per le scritture è l'eco del comando, per le letture contiene i dati.

## Formato del pacchetto

| Byte | Significato |
|---|---|
| 0 | Report ID, sempre `0x04` |
| 1-2 | Checksum little-endian: somma dei byte 3..63 |
| 3 | Comando |
| 4 | Lunghezza dati (max `0x38` = 56) |
| 5-6 | Offset little-endian |
| 7 | `0x00` |
| 8.. | Dati |

## Comandi

| Cmd | Direzione | Uso |
|---|---|---|
| `0x01` | scrittura | Inizio transazione (`04 01 00 01`) |
| `0x02` | scrittura | Fine transazione e salvataggio (`04 02 00 02`) |
| `0x03` | lettura | Info dispositivo (44 byte) |
| `0x05` | lettura | Configurazione corrente (effetto, luminosità, ecc.) |
| `0x06` | scrittura | Scrive la configurazione all'offset indicato |
| `0x07` | lettura | Keymap estesa (più layer), ~1100 byte |
| `0x08` | scrittura | Keymap estesa (stessi offset di `0x07`) |
| `0x09` | lettura | Area macro (intestazione `aa 55` + lunghezza) |
| `0x0A` | scrittura | Area macro, tutta intera |
| `0x0F` | lettura | Keymap del layer base: 126 slot × 3 byte = 378 byte |
| `0x11` | scrittura | Colori per tasto: 144 slot × 3 byte = 432 byte, a blocchi da 54 (`0x36`) |

### Info dispositivo (`0x03`)

`55 aa ff 02 | 45 0c | 2f 65 | 02 01 00 08 ...` seguito dall'elenco degli effetti supportati
(`01 02 03 04 05 06 08 07 09 0b 0a 0c 0d 0e 0f 10 11 12 14`, terminato da `ff`).
Contiene VID e PID little-endian, utili per il riconoscimento automatico del modello.

**Il blocco si può riscrivere** con `0x04` (vedi "Altro" più sotto), e non contiene sempre
l'elenco: dal 29 ottobre 2026, dopo il test dei limiti delle macro con l'app ufficiale, sulla
nostra tastiera `0x03` restituisce `05 a2 a2 00 … 01 01 02 01 01 02 01 01 02 00 00 00 01 01 02`
(niente firma `55 aa ff 02`, niente VID/PID). Il blocco è cambiato tra le 12:27 e le 12:52,
fuori dalla cattura `06`, che finisce alle 12:47: con ogni probabilità è stata l'app ufficiale
a riscriverlo alla chiusura. La tastiera funziona normalmente e tutti gli effetti partono
comunque. Per questo l'app accetta l'elenco solo con la firma e gli ID giusti; altrimenti
mostra tutti gli effetti conosciuti.

### Configurazione (`0x05` / `0x06`)

| Offset | Valore letto | Significato |
|---|---|---|
| `0x00` | `14` | Effetto (`0x14` = colori per tasto) |
| `0x01` | `04` | Luminosità 0..4 |
| `0x02` | `03` | Velocità |
| `0x03` | `00` | Direzione |
| `0x04` | `00` | Colore casuale |
| `0x05-0x07` | `ff ff ff` | Colore R G B |
| `0x0F` | `03` | Report rate USB: `00` 125 Hz, `01` 250 Hz, `02` 500 Hz, `03` 1000 Hz (cattura `03`) |
| `0x11` | `03` | da capire |
| `0x13` | `01` | da capire: l'app lo scrive a 1 a ogni avvio, prima di inviare i colori. **Non** decide se i colori restano: vedi sotto |

## Effetti hardware (cattura `02_effetti`, ottobre 2026)

L'app ufficiale scrive **un byte alla volta**, ognuno nella sua transazione `0x01` … `0x02`,
a ogni scatto dei cursori. La configurazione è unica: velocità, luminosità e colore non sono
salvati per effetto.

| Azione nell'app | Scrittura `0x06` |
|---|---|
| Scelta dell'effetto | offset `0x00` = codice effetto, poi offset `0x04` = `00` (casuale spento) |
| Luminosità | offset `0x01`, valori `00`..`04` |
| Velocità | offset `0x02`, valori `00`..`03` (4 scatti nell'app) |
| Direzione | offset `0x03`, `00` oppure `ff` (non `01`) |
| Colore | offset `0x04` = `00`, poi 3 byte R G B all'offset `0x05` |
| Colori casuali | offset `0x04` = `01` (acceso) / `00` (spento) |
| Ritorno a *Custom* | solo offset `0x00` = `14`: i colori per tasto restano nella tastiera |

Codici effetto, nell'ordine del menu dell'app: Wave #1 `01`, Wave #2 `02`, Spiraling wave `03`,
Acid Mode `04`, Breathing `05`, Normally on `06`, Ripple graff `08`, Pass without trace `07`,
Fast run without trace `09`, Matrix 3 `0b`, Matrix 2 `0a`, Rainbow `0c`, Heartbeat Sensor `0d`,
Kamehameha `0e`, Digital Times `0f`, Ping Pong `10`, Surmount `11`, Glorious Mode `12`, Custom `14`
(stesso ordine dell'elenco nella risposta `0x03`; verificati in cattura `01`, `05`, `06`, `07`, `14`).

Cambiando effetto la tastiera fa da sé una transizione a onda: l'app manda solo il codice.

Verificato a vista: la velocità `00` è la più veloce e `03` la più lenta; la direzione `ff` va verso
sinistra e `00` verso destra. Pass without trace illumina il tasto premuto, Fast run without trace
la sua riga. Direzione: solo Wave #1, Wave #2 e Spiraling wave. Colore e colori casuali: tutti
tranne Acid Mode e Surmount, che cambiano colore da soli. Alcuni effetti (di sicuro Ping Pong e
Surmount) reagiscono a quanto si scrive: è un comportamento del firmware, non un parametro.

## Sequenza dell'app per i colori per tasto

1. `0x01` inizio, `0x06` offset `0x13` = `01`, `0x02` fine.
2. `0x01` inizio, poi 8 pacchetti `0x11` (offset `0x000`, `0x036`, ... `0x17A`) per un totale di 432 byte, poi `0x02` fine.

L'app non invia il comando di cambio effetto in questa transazione, e manda sempre tutti i 432 byte.

**I colori per tasto restano nella tastiera** anche staccandola (prova del 29 ottobre 2026 con
`lucent-dev --test-13`): sia con `0x13 = 01` sia con `0x13 = 00` il solo comando `0x11`, senza
riscrivere la modalità, basta perché al ricollegamento tornino gli ultimi colori inviati. Quindi
ogni invio finisce nella memoria permanente e non c'è (finora) un modo "temporaneo": per questo
l'app non cambia profilo da sola in base al programma in uso.

**Luminosità nella modalità per tasto:** anche con luminosità 4, i LED sono molto più tenui se la
configurazione ha velocità `00` e colore `00 00 00`. Con i valori che lascia l'app ufficiale
(`14 04 03 00 00 ff ff ff`) raggiungono la piena luminosità (verificato a vista, ottobre 2026).
Non è ancora chiaro quale dei due byte conti, la velocità o il colore.

**Più programmi aperti:** ogni handle HID riceve tutte le risposte della tastiera, anche quelle
ai comandi inviati da un altro programma. Prima di ogni comando vanno scartate le risposte in coda,
e vanno ignorate quelle con un byte di comando diverso.
Il codice ereditato inviava solo 378 byte e cambiava effetto nella stessa transazione: è il sospetto principale del perché i colori per tasto non funzionavano.

## Report rate e mappa dei tasti (cattura `03_tasti_macro`, ottobre 2026)

**Report rate.** Ogni cambio nell'app è una transazione con un solo byte di configurazione,
come gli effetti: `01` inizio, `06` offset `0x0F` lunghezza 1, `02` fine.
Valori scritti in ordine nella cattura: `00` (125 Hz), `01` (250), `02` (500), `03` (1000).

**Lettura all'avvio dell'app.** Info (`0x03`), keymap base (`0x0F`, 378 byte), keymap estesa
(`0x07`, 1134 byte = 3 × 378 letti in blocchi di 56 e 42 byte), poi due volte `0x09` (16 byte).

- Primo terzo di `0x07`: identico alla keymap base.
- Secondo terzo (offset `0x17A`): in gran parte zeri, poi di nuovo la keymap base da `0x208`;
  terzo terzo: di nuovo la keymap base. Il significato dei layer va confermato con una
  riassegnazione catturata.
- Oltre la lunghezza richiesta, le risposte contengono resti del buffer precedente
  (es. `01 04 02 00 ff`): vanno ignorati.
- `0x09` risponde `aa 55 10 00 00 00 01 00 00 00 00 00 30 00 30 00`: forse l'intestazione
  dell'area macro, da verificare.

**Macro e riassegnazioni**: nessuna scrittura in questa cattura. Nell'app ufficiale si possono
riprogrammare i tasti solo dopo aver messo l'illuminazione su *Disabled*: vedi la cattura `04`.

## Tasti e macro (cattura `04_macro`, ottobre 2026)

Ogni modifica è **una transazione**: `01` inizio, `0A` area macro intera, `08` keymap, `02` fine.
L'app riscrive sempre tutta l'area macro e il layer 0 (7 pacchetti, offset `0x000`..`0x150`);
alla prima modifica ha scritto anche i layer 1 e 2 (`0x17A`..`0x46D`).

**Keymap (`0x08`)**: stesso formato degli slot di `0x0F` (vedi sotto). Attenzione: dopo una
scrittura `0x0F` continua a rispondere con la keymap **di fabbrica**; quella in uso si legge
con `0x07` (primo terzo = layer 0). Nella cattura:

- Bloc Maiusc (slot 52): `02 02 39` → `02 02 60`, poi ripristinato.
- Pausa (slot 108) e Freccia sinistra (slot 94): `05 01 00` = **macro n. 0**.
- Pausa: `03 e9 00` = multimediale Volume su (usage consumer `0x00E9`).

**Area macro (`0x09` lettura, `0x0A` scrittura)**, little-endian, verificata anche con la
cattura `05_macro_ctrl` (due macro, una con Ctrl) e sulla tastiera:

| Byte | Esempio | Significato |
|---|---|---|
| 0-1 | `aa 55` | Firma |
| 2-3 | `48 00` | Lunghezza totale dell'area (vuota: `10 00` = 16) |
| 4-5 | `02 00` | Numero di macro |
| 6-7 | `01 00` | Sempre `01 00` |
| 8-9 | `1b 00` | Non chiaro: 2 × passaggi totali + 11 nelle catture; vuota: `ff ff` (e `ff` al byte 10) |
| 10-15 | `00` | — |
| 16.. | `14 00 2e 00` | **Tabella**: posizione di ogni macro dall'inizio dell'area (u16) |

Ogni macro, alla sua posizione:

| Campo | Esempio | Significato |
|---|---|---|
| u16 | `04 00` | Numero di passaggi |
| u8 | `01` | Ripetizioni |
| u8 | `03` | Lunghezza del nome in caratteri |
| 4 byte × passaggi | `4c a0 01 01` | Pausa dopo il passaggio (unità di 10 ms), `a0` premuto / `20` rilasciato, tipo, codice |
| nome | `75 00 6e 00 6f 00` | UTF-16LE (`uno`) |

Il tipo del passaggio è come nella keymap: `02` + codice HID per i tasti, `01` + bit per i
modificatori (`01 01` = Ctrl sinistro). Una pausa di 100 unità dura circa un secondo.
Tasto disattivato nella keymap: `00 00 00` (verificato).

**Layer 1 e 2**: ora contengono soprattutto `00 00 00` (forse "come il layer 0") e pochi valori
(`02 02 00`, `01 01 02`) da interpretare. Letti prima della scrittura (cattura `03`) erano dati
spostati e senza senso: probabilmente non inizializzati. Gli slot 118-125 di tutti i layer
contengono resti di buffer, che l'app riscrive così come sono.

**Altro**: uscendo, l'app ha mandato `0x04` con gli stessi 44 byte della risposta `0x03`. Significato non noto.

## Limiti delle macro (cattura `06_macro_limiti`, ottobre 2026)

L'app ufficiale ha scritto prima una macro da **186 passaggi**, poi **17 macro** (quella lunga più
16 da due passaggi) in un'area di **990 byte**. Riletta dalla tastiera, l'area c'era tutta, con la
macro 0 su Pausa. L'app ufficiale non pone limiti mentre si registra, ma all'ultimo *Apply* ha avvisato
*"shortage of hardware space, cancelled the macro name of storage"*: lo spazio non bastava e ha
scritto le macro **senza nomi** (infatti nella cattura sono vuoti). Con i nomi ("Macro1"…"Macro17",
circa 200 byte) l'area sarebbe stata intorno ai 1200 byte, quindi lo spazio vero sta tra 990 e
~1200 byte: probabilmente **1024**, ma non è provato. L'app usa come massimi i valori dimostrati:
17 macro, 186 passaggi, 990 byte.

In questa cattura i byte 6-15 dell'intestazione erano `00 00 6f 0f 76 cc 00 2a 00 80` (anche con
una macro sola): l'app ufficiale non li inizializza, e il firmware li ignora. Fn non si può
registrare in una macro: lo gestisce la tastiera, non arriva al PC.

Backup letti dopo la cattura: `captures/backup/layers-*.bin` e `macros-*.bin` (`lucent-dev --read-keys`).

## Slot dei tasti

Keymap (`0x0F`) e colori (`0x11`) usano lo **stesso indice di slot**: gli slot senza tasto fisico sono neri nel buffer colori.
Ogni slot della keymap è di 3 byte:

- `02 02 xx`: tasto normale, `xx` = codice HID (es. `29` Esc, `04` A).
- `02 01 xx`: modificatore, `xx` = bit (`01` LCtrl, `02` LShift, `04` LAlt, `08` LWin, `10` RCtrl, `20` RShift, `40` RAlt, `80` RWin).
- `03 lo hi`: tasto multimediale (consumer), es. `03 e2 00` = Muto.
- `05 02 02`: tasto Fn.
- `05 a2 xx`: slot senza tasto (a 0, 17, 34, 51, 68, 85).
- `05 01 nn`: macro numero `nn` (cattura `04`).

| Riga | Slot |
|---|---|
| Esc, F1..F12 | 1, 2..13 |
| `\` 1..0 ' ì | 18, 19..28, 29, 30 |
| Bloc Num, / *, tastierino | 31, 32, 33 |
| Tab Q W E R T Y U I O P è + | 35, 36..45, 46, 47 |
| Tastierino 7 8 9 | 48, 49, 50 |
| Bloc Maiusc A S D F G H J K L ò à ù | 52, 53..61, 62, 63, 64 |
| Tastierino 4 5 6 | 65, 66, 67 |
| Shift sx, Z..- , Shift dx, Invio | 69, 70..79, 80, 81 |
| Tastierino 1 2 3 | 82, 83, 84 |
| Ctrl Win Alt Spazio AltGr Fn Menu Ctrl | 86, 87, 88, 89, 90, 91, 92, 93 |
| Frecce ← ↓ ↑ → | 94, 95, 96, 97 |
| Backspace | 98 |
| Tastierino 0 . Invio | 99, 100, 101 |
| Tasto ISO `<` | 105 |
| Stamp, Bloc Scorr, Pausa | 106, 107, 108 |
| Ins, Home, PgSu, Canc, Fine, PgGiù | 110..115 |
| Tastierino - + | 116, 117 |

Gli slot 14-16 (multimediali) e 102-104 sono neri nel buffer colori: probabilmente non hanno un LED fisico.
L'ordine dei colori sembra RGB (lettere `ff 00 00`, resto `00 ff ff`), da confermare a vista.
