# Lingue

Ogni lingua è un file `.ini` nella cartella `lang/` accanto all'eseguibile. Italiano e inglese
sono inclusi nell'app e vengono riscritti a ogni avvio (i sorgenti sono in `lang/` nel repository).

## Aggiungere una lingua

1. Copia `lang/en.ini` e rinomina la copia con il codice della lingua: `es.ini`, `de.ini`, `pt-br.ini`…
2. In `[meta]` scrivi il nome della lingua nella lingua stessa (`name = Español`).
3. Traduci il testo dopo ogni `=`. Lascia invariati i segnaposto tra graffe (`{name}`, `{ms}`…);
   `\n` va a capo. Le righe che togli restano in inglese.
4. Riapri le Impostazioni: la lingua compare nell'elenco.

In **Automatica** l'app usa la lingua di Windows (prima la variante esatta, es. `pt-br`, poi la
lingua base, `pt`), altrimenti l'inglese.

## Per chi sviluppa

- Interfaccia: `t("sezione.chiave", { segnaposto })` da `ui/src/lib/i18n.svelte.ts`; `plural()` usa
  le chiavi `_one` / `_other`. Backend: `i18n::tr("backend.chiave", &[("nome", &valore)])`.
- Ogni chiave nuova va in `en.ini` **e** `it.ini`: un test (`cargo test -p lucent`)
  fallisce se le due lingue incluse non hanno le stesse chiavi.
- Gli strumenti MCP e il registro attività non passano dai file di lingua.
