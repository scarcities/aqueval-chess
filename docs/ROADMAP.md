# Roadmap für aqueval chess: Recherche-Ideen einbauen

Stand 21.10.2026, geprüft an `main` 44e7eea. Grundlage: der validierte Bericht „Epsylons Elo-Fahrplan“ und ein Code-Audit (nur gelesen). Alle Ideen sind in eigenen Worten beschrieben; es wird kein Code anderer Engines übernommen. Umsetzung und Tests: Thread „Engine-Entwicklung übernehmen“.

**Kurzfassung.** Erst die laufende Queue zu Ende bringen und dabei drei Fehler im Fundament beheben, die das Audit gefunden hat: Der TT überschreibt bei jeder Kollision, Wiederholungen zählen in der Suche erst beim dritten Mal als Remis, und die Suche ist durchgehend fail-hard. Danach kommt das eigene NNUE (Generation 0), weil es pro Rechenstunde mit Abstand am meisten Elo bringt: grob +150 bis +300 Elo für etwa anderthalb Tage Maschinenzeit, gegen etwa +5 Elo pro Suchtest. Die großen eval-unabhängigen Suchtechniken (Singular Extensions, History Pruning, History-Paket, Capture History neu, ProbCut, Correction-History-Leiter) folgen auf dem NNUE-Stand, weil ihre Margins dann gleich auf der richtigen Eval-Skala sitzen. SPSA-Tuning kommt erst danach. Das Testprotokoll wechselt auf **[0, 10] nElo mit Cap 25.000**; das sind etwa [0; 5,8] in klassischem Elo und fast doppelt so viele Tests pro Tag wie [0, 5] nElo bei kaum weniger eingesammeltem Elo.

**Lesehinweis.** Elo-Erwartungen sind markiert: **M** = gemessen in einer anderen Engine (Quelle im Bericht), **S** = eigene Schätzung für aqueval chess. „nElo“ ist normalized Elo, die Einheit, in der fastchess und `scripts/run_sprt.sh` (`model=normalized`) die Grenzen rechnen. Bei aqueval chess bei STC mit UHO-Buch gilt etwa 1 Elo ≈ 1,73 nElo. Zeilenangaben beziehen sich auf `main` 44e7eea.

## Testprotokoll ab dem Test nach IIR

| Art der Änderung | Grenzen | Cap | Bedenkzeit | Mittlere Dauer (S) |
|---|---|---|---|---|
| Gainer (Standard für alle Features) | [0, 10] nElo ≈ [0; 5,8] Elo | 25.000 Spiele (`ROUNDS=12500`) | 8+0,08 | ~3,7 h |
| Enabler, Vereinfachung, reiner Speedup | [−8, 0] nElo ≈ [−4,6; 0] Elo | 25.000 | 8+0,08 | ~5,8 h |
| Kleine Verfeinerung (erwartet +1 bis +3) | bündeln oder in den SPSA-Block; einzeln [0, 8] nElo | 25.000 | 8+0,08 | ~4,8 h |
| Zeitmanagement | [0, 10] nElo | 25.000 | 20+0,2 | ~9 h |
| NNUE-Build gegen HCE-Main | [0, 10] nElo | 25.000 | 8+0,08 | < 1 h bei großem Gewinn |
| Netz gegen Netz (ab Gen-1) | [0, 10] nElo | 25.000 | 8+0,08 | ~3,7 h |
| LTC-Kontrolle (kein SPRT) | festes Match, 2.000 Spiele | – | 40+0,4 | ~5 h, nach jeder Phase |

**Warum [0, 10] nElo statt [0, 5] nElo.** Eigene Simulation (GSPRT, α = β = 0,05, Pentanomial, Remisquote ~50 %, 1.700 Spiele/h) über 30 Roadmap-Ideen: [0, 5] nElo mit Cap 25k sammelt 92 % des verfügbaren Elo bei 7,1 h pro Test, [0, 10] nElo mit Cap 25k 88 % bei 3,7 h. Bei wahren +3 / +4 / +5 / +6 Elo nimmt [0, 10] nElo zu 50 / 71 / 88 / 96 % an; ein Patch mit wahren 0 Elo kommt zu 4–5 % durch, einer mit −3 Elo zu unter 0,5 %. Das bisherige „[0, 5]“ war wegen der nElo-Einheit in Wahrheit etwa [0; 2,9] Elo und deshalb so langsam. [−3, 1] ist als Nicht-Regressionstest ungeeignet: In nElo besteht eine echte 0-Elo-Vereinfachung nur zu 21 %.

**Weiter gültig:** eine SPRT zur Zeit; jeder Test gegen das aktuelle `main`; vor jeder SPRT `cargo test --release`, Perft und Bench; Stopp nach drei Fehlschlägen oder Inconclusives in Folge (dann pausieren und melden). Concurrency bleibt 10; beide Seiten teilen sich die SMT-Kerne gleich. Für die Zeitmanagement-Tests vorher und nachher Zeitverluste in den PGNs zählen: Ziel null. In AGENTS.md jede Grenze mit Einheit schreiben („[0, 10] nElo“).

**Laufender IIR-Test:** bleibt wie gestartet ([0, 5] nElo, Cap 15.000) und wird so geloggt. Endet er am Cap mit positivem Elo, später eine verbesserte Variante als neue Hypothese testen, nicht denselben Patch neu starten.

## Audit: die acht Checks am echten Code

| Check | Ergebnis | Fundstelle | Folge |
|---|---|---|---|
| Re-Search-Reihenfolge nach LMR | in Ordnung: erst volle Tiefe mit Nullfenster, dann volles Fenster nur bei alpha < score < beta | `src/search/mod.rs:402-411` | keine |
| Rückgabewert bei TT-Cutoffs | in Ordnung: TT-Wert wird zurückgegeben, Cutoffs nur im Nullfenster. Der Rest der Suche ist aber fail-hard (`return beta` in NMP, Beta-Cutoff und QS; TT speichert beta statt des echten Werts) | `mod.rs:234-247`; fail-hard: `mod.rs:286`, `:372-373`, `:441`, `:462-463` | Schritt 1.4 Fail-soft |
| 50-Züge-Regel bei TT-Cutoffs | fehlt: kein Schutz bei hohem Halbzugzähler | `mod.rs:228`, `:234` | in Schritt 1.1 mitnehmen |
| Move Overhead, movestogo, harte Grenze | in Ordnung: Overhead wird abgezogen, movestogo geparst, harte Grenze höchstens 80 % der Restzeit. Aber: neue Iteration nur, solange weniger als die Hälfte des weichen Limits verbraucht ist; aqueval chess nutzt damit deutlich weniger Zeit als zugeteilt | `src/uci/mod.rs:184-192`, `src/search/time.rs:50-60`, `mod.rs:632-635` | Schritt 1.6 Zeitmanagement |
| LMP-Zähler | in Ordnung: zählt über den Listenindex, also inklusive geprunter Züge und Schläge | `mod.rs:321` | keine |
| Quiet Checks in der QS | keine vorhanden. Aber die QS steht auch im Schach auf der statischen Eval und übergeht stille Umwandlungen | `mod.rs:439-443`, `:450` | QS-Paket in Phase 3 |
| Stack-Sentinels | in Ordnung: alle Rückblicke über `checked_sub`/`Option`, Ply-Grenze vor den Tabellen | `mod.rs:143-152`, `:223`, `:255-260` | Muster für ContHist Ply 4/6 beibehalten |
| Bench-Signatur und Hardware | Bench 888666 (AGENTS.md); Der Referenzrechner hat 6 Kerne mit SMT, Concurrency 10 teilt also Kerne | AGENTS.md, `scripts/run_sprt.sh` | Concurrency bleibt; Zeitverluste prüfen |

### Zusatzfunde des Audits

| Fund | Fundstelle | Folge |
|---|---|---|
| **TT überschreibt bei jeder Kollision.** Ein Slot pro Index; ein anderer Schlüssel ersetzt immer, auch einen viel tieferen Eintrag | `src/search/tt.rs:30`, `:74` | Schritt 1.2 TT-Buckets, Voraussetzung für Singular Extensions |
| **Wiederholung zählt in der Suche erst dreifach.** Eine einmalige Wiederholung innerhalb des Suchbaums wird nicht als Remis gewertet | `src/board/mod.rs:151-172` (count ≥ 3), Aufruf `mod.rs:228` | Schritt 1.1 Remis-Hygiene |
| **Butterfly-History ohne Malus und ohne Gravity.** Nur Bonus depth², gedeckelt bei 700.000; häufige Züge sättigen und verlieren die Unterscheidung. Alle Heuristik-Tabellen werden bei jedem `go` neu angelegt | `mod.rs:116-124`, `:509` | History-Paket in Phase 3 |
| QS erzeugt alle legalen Züge und filtert dann Schläge; kein TT in der QS | `mod.rs:446-450` | QS-Paket in Phase 3 |
| Sortierung berechnet SEE für jeden Schlag und sortiert die ganze Liste vorab | `mod.rs:180`, `:304-311` | später gestufter Move Picker (Speedup) |
| EP-Feld wird nach jedem Doppelschritt gehasht, auch ohne möglichen EP-Schlag (TASKS P2-01 offen) | `src/board/make_move.rs:105-112` | Hygiene-Bündel in Phase 3 |
| Root verwirft eine abgebrochene Iteration ganz; Info zeigt nur einen PV-Zug | `mod.rs:598-604`, `:618` | in Schritt 1.6 prüfen; PV-Ausgabe kosmetisch |
| Threads maximal 1 | `src/uci/options.rs:59` | erst vor einer CCRL-Einreichung |

## Reihenfolge

### Phase 0: sofort, ohne Rechenzeit (während IIR läuft)

- `scripts/run_sprt.sh`: Gainer-Default auf [0, 10] nElo, Vereinfachung auf [−8, 0] nElo, `ROUNDS=12500` als Default (Cap 25.000). `model=normalized` bleibt explizit stehen. AGENTS.md: Testprotokoll oben übernehmen, Einheit nElo überall.
- Zeitverluste in den vorhandenen SPRT-PGNs zählen.
- Branches für 1.1 bis 1.4 im Worktree vorbereiten (eigene Implementierung, kein fremder Code).

### Phase 1: Queue zu Ende bringen, Fundament reparieren (~2 Tage Maschinenzeit)

| Schritt | Inhalt | Test | Erwartung | Warum hier |
|---|---|---|---|---|
| 1.0 | NNUE-Infrastruktur mergen (Patches 0001–0005 aus `nnue-prep`, `git am -3`) | kein SPRT, wenn Default-Build gleiche Bench-Signatur hat und NPS (8 Läufe je Seite, Maschine sonst leer) innerhalb 1 % liegt; sonst [−8, 0] nElo | 0 Elo, kein Verhalten geändert | Jeder spätere Suchpatch muss sonst neu an die Evaluator-Schnittstelle angepasst werden; einmal früh mergen spart Konflikte |
| 1.1 | Remis-Hygiene: eine Wiederholung nach der Wurzel zählt als Remis, dreifach nur über die Partiegeschichte; keine TT-Cutoffs ab etwa 90 Halbzügen | [0, 10] nElo | S: +3 bis +10 | Fehler im Fundament; verändert jeden Baum, also vor allen weiteren Tests |
| 1.2 | TT-Buckets: mehrere Einträge pro Cache-Zeile, Ersetzung nach Tiefe und Alter, tiefe Einträge bleiben | [0, 10] nElo | M: Weiss (HCE) +17,0 / +6,6 für ein neues Ersetzungsschema. S: +10 bis +25, weil heute jede Kollision überschreibt | Voraussetzung für Singular Extensions; hilft allen folgenden Tests |
| 1.3 | Pawn Correction History (Queue-Schritt 11) | [0, 10] nElo | M: Weiss (HCE) +4,9 / +11,7 | wie geplant |
| 1.4 | Fail-soft: Beta-Cutoffs, NMP und QS geben den echten Wert zurück, der TT speichert ihn | [−8, 0] nElo (Enabler) | S: 0 bis +5 | AGENTS.md: Aspiration „worth retrying after a fail-soft search“ |
| 1.5 | Aspiration-Retest auf Fail-soft (Queue-Schritt 13), Fenster wächst bei Fail, nach Fail-high flacher nachsuchen | [0, 10] nElo | M: Weiss (HCE) +14,1 / +8,1, Ethereal (HCE) +10,6 / +4,8 für Verfeinerungen eines bestehenden Fensters. S: +4 bis +10 | braucht 1.4 |
| 1.6 | Zeitmanagement (Queue-Schritt 12) bei 20+0,2: weiches Limit wirklich nutzen statt nach der Hälfte aufzuhören; abgebrochene Iteration verwerten, wenn der erste Zug fertig ist | [0, 10] nElo, 20+0,2 | S: +10 bis +30, weil heute viel Zeit liegen bleibt | Basis für die TM-Zusätze in Phase 3 |
| 1.7 | Gauntlet „HCE final“ | wie bisher (`scripts/elo_gauntlet.sh`) | – | Meilenstein vor dem NNUE |

### Phase 2: NNUE Generation 0 (~1,5 Tage Maschinenzeit, keine SPRT parallel)

| Schritt | Inhalt | Dauer (S) |
|---|---|---|
| 2.1 | Datagen mit dem `main` nach Phase 1: 100 Mio. Stellungen, 5.000 Knoten pro Zug, zufällige Eröffnungszüge, Filter wie in `docs/NNUE.md` | ~14 h mit 10 Threads (~7 Mio./h) |
| 2.2 | Training mit dem eigenen Trainer von aqueval chess (`scripts/nnue/train.py`, CPU-PyTorch im venv, ~200 MB Download) oder, falls eine CUDA/ROCm-GPU da ist, mit bullet als Bibliothek in einem eigenen Trainingsprogramm | ~6–8 h CPU für 40 Epochen |
| 2.3 | Sanity-Checks aus `docs/NNUE.md`, Eval-Skala auf Centipawns normieren, dann SPRT NNUE-Build gegen HCE-`main` | [0, 10] nElo; M: akimbo +388, Stormphrax +358 (gegen die HCE-Engine des Autors), Leorik ~+305. S: +150 bis +300 |
| 2.4 | Bei Erfolg: Netz einbetten, NNUE als Default, Gauntlet „NNUE Gen-0“ | ~1 h |

Nur selbst erzeugte Daten, kein fremdes Netz und keine fremden Datensätze. Das hält aqueval chess eigenständig und erfüllt die TCEC-NNUE-Richtlinie.

### Phase 3: Suchstruktur auf dem NNUE-Stand (~3–4 Tage)

Alle Schritte [0, 10] nElo, wenn nicht anders angegeben. Reihenfolge nach Erwartung, Abhängigkeiten und Aufwand.

| Schritt | Inhalt | Erwartung |
|---|---|---|
| 3.1 | Singular Extensions (Basis, Ausschlusszug-Suche, Begrenzung pro Pfad) | M: Weiss (HCE) +11,5 / +24,2 |
| 3.2 | Multicut aus der Singular-Suche | M: Weiss +4,8 / +3,2 |
| 3.3 | Doppelte und negative Extensions (gebündelt) | M: Weiss doppelt +11,5 / +11,2, negativ +2,7 / +3,1 |
| 3.4 | History-Paket: Gravity und Malus für die Butterfly-History, Tabellen über Züge hinweg behalten (bei neuem `go` abschwächen statt löschen) | S: +5 bis +15 (Audit-Fund) |
| 3.5 | History Pruning für stille Züge mit sehr schlechter History bei geringer Tiefe | M: Weiss (HCE) +9,1 / +10,5 |
| 3.6 | Capture History neu als Paket: SEE nur für gut/schlecht, innerhalb der Gruppen Opferwert plus Capture History, Malus für alle versuchten Schläge, Capture History verschiebt die SEE-Schwelle | S: +3 bis +8; die zwei Fehlschläge lagen vermutlich am Design |
| 3.7 | LMR-Paket: Reduktion nach History, weniger in PV/ttPv-Knoten, mehr an Cut Nodes (Cut-Node-Flag), tiefer/flacher nachsuchen | M: Weiss +2,3 / +5,9, Viridithas +5,9. S: +5 bis +10 gesamt |
| 3.8 | QS-Paket: TT in der QS lesen und schreiben, Schachgebote in der QS mit Ausweichzügen beantworten; nur Schläge erzeugen (Speedup, [−8, 0] nElo) | M: Weiss +5,4 / +6,9 für TT in der QS |
| 3.9 | ProbCut mit QS-Vorprüfung | M: Weiss (HCE) +6,4 / +6,7 |
| 3.10 | Correction-History-Leiter: Non-Pawn, Minor/Major, Continuation (je ein Test) | M: Weiss Non-Pawn +2,8 / +6,8; StockDory Continuation +3,6 / +5,5 |
| 3.11 | Cuckoo-Erkennung kommender Wiederholungen | M: Weiss +3,6 / +5,4 |
| 3.12 | Hindsight-Reduktionen (Stockfish 2025, gebündelt) | M: SF je ≈ +1 bis +2 |
| 3.13 | TM-Zusätze bei 20+0,2: Stabilität des besten Zugs, dann Node-Fraction | M: StockDory +6,9 / +9,1; rusty-rival +4,5 |
| 3.14 | Hygiene-Bündel: EP-Hash nur bei möglichem EP-Schlag, Mate Distance Pruning, NMP-Verifikation bei großer Tiefe | [−8, 0] nElo |

### Phase 4: Tuning (~2 Tage)

- SPSA über alle Suchkonstanten auf dem NNUE-Stand, 30.000–60.000 Spiele. fastchess hat kein SPSA; dafür ein kleines eigenes Treiberskript nach dem bekannten Verfahren (OpenBench-Defaults als Startwerte: α 0,602, γ 0,101, A = 10 % der Iterationen). Danach das Ergebnis gegen den Vor-Tune-Stand testen ([0, 10] nElo) und mit einer LTC-Kontrolle bestätigen.
- Erst hier, weil die Margins von RFP, Futility, ProbCut und Razoring an der Eval-Skala hängen und nach dem NNUE ohnehin neu sitzen müssen.

### Phase 5: NNUE-Generationen (laufend, nachts)

Gen-1-Daten mit dem NNUE-Engine (200 Mio.+), dann breitere erste Schicht (512, später 1024), King Buckets mit Accumulator-Cache, Output-Buckets nach Material, AVX2-Lazy-Updates. Jede Stufe Netz gegen Netz [0, 10] nElo. Threat-Inputs erst bei großen Netzen.

### Phase 6: eigene Forschungslinie (nebenher)

Logging-Infrastruktur für Knotendaten, daraus aus eigenen Daten gefittete Margins und LMR, unsicherheits-skalierte Margins über den Betrag der Correction History, Gini-Signal fürs Zeitmanagement, Vergleich zweier Datenlinien (aus HCE gestartet gegen aus Zufallsnetz gestartet). Alles mit demselben Testprotokoll.

### Bewusst nicht

MCTS- oder Hybrid-Suche, Policy-Netze fürs Move Ordering, suchfreie Transformer, Texel-Tuning der HCE (wird durch das NNUE ersetzt), Lazy SMP vor einer CCRL-Einreichung, fremde Netze oder Trainingsdaten.

## Warum diese Reihenfolge

1. **Elo pro Rechenstunde.** Die Maschine ist der Engpass. Ein Suchtest bringt im Mittel etwa +4 Elo in ~3,7 h; NNUE Gen-0 bringt nach den Messungen anderer Engines +150 bis +300 Elo in ~1,5 Tagen. Deshalb kommt das NNUE vor die große Suchwelle und nicht ans Ende.
2. **Fundament zuerst.** Wiederholungen, TT-Ersetzung und Fail-soft verändern jeden Suchbaum. Solange sie fehlerhaft sind, misst jeder weitere Test gegen ein verzerrtes Main. Der TT muss außerdem vor Singular Extensions stimmen, weil diese tiefe Lower-Bound-Einträge brauchen.
3. **Abhängigkeiten.** Fail-soft vor Aspiration (Notiz in AGENTS.md); TT vor Singular Extensions; Cut-Node-Flag vor dem LMR-Paket; Zeitmanagement-Basis vor den TM-Zusätzen.
4. **Eval-Skala.** Alles, was an Margins hängt, und das SPSA-Tuning kommt nach dem NNUE. Eval-unabhängige Struktur (Extensions, TT, History, Wiederholungen) wandert unverändert mit.
5. **NNUE-Infrastruktur früh mergen.** Der Default-Build ändert sich nicht (gleiche Bench-Signatur). Wer sie erst nach zehn weiteren Suchpatches einspielt, muss jeden davon an die Evaluator-Schnittstelle anpassen.
6. **Datengenerierung erst nach Phase 1.** Bessere Remis- und TT-Behandlung verbessern die Labels und Partieergebnisse der Trainingsdaten; die Verzögerung beträgt nur etwa zwei Tage.

## Validierung des Berichts

Zwei unabhängige Durchgänge haben rund 200 Aussagen des Berichts gegen ihre Quellen geprüft: etwa 165 bestätigt, etwa 30 korrigiert, etwa 25 nicht prüfbar (markiert), keine falsch. Keine Korrektur ändert die Reihenfolge der Top 10.

- **Bestätigt** sind alle Zahlen, auf denen die Rangfolge beruht: Weiss Singular Extensions +11,52 / +24,21, TT-Ersetzung +16,98 / +6,63, TT in der QS +5,41 / +6,89, History Pruning +9,05 / +10,50, ProbCut +6,36 / +6,65, Pawn-CorrHist +4,87 / +11,70; StockDory Zeitmanagement +6,85 / +9,10; Viridithas +29,59 und +14,41; akimbo +387,57; Stormphrax +357,65; TCEC-NNUE-Richtlinie; bullet ohne CPU-Backend.
- **Korrigiert:** sechs Ethereal-Paare mit vertauschtem STC/LTC oder Zahlen aus Nachbar-Commits (u. a. ProbCut-Vorprüfung +2,61 / +6,10, LMP-Zähler +4,79 / +6,31); Stockfish-Optimism (STC wurde abgelehnt, nicht „unentschieden“); Tetrarchs +63,4 gilt für ein ganzes Release; Threat-Inputs kamen bei Viridithas mit v19; Leorik +355 enthält ~50 Elo Nicht-NNUE-Änderungen; Stormphrax +358 ist gegen die frühere HCE-Engine des Autors gemessen.
- **SPRT-Einheit:** Die Wahrscheinlichkeiten im Bericht gelten für logistische Grenzen. aqueval chess testet in nElo; daraus folgt das neue Protokoll oben. Die mittleren Testlängen im Bericht waren um ~11 % zu hoch (Wald-Formel statt B/μ).

Die korrigierte Fassung des Berichts und beide Prüfprotokolle liegen in `epsylon-research/` (Projektdateien).
