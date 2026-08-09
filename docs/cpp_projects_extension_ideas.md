# Rozszerzenie analizatora na projekty spoza czystego QML (notatki)

Analiza projektów w `temp/` (Ambxst, DankMaterialShell, WhiteSur-kde, cool-retro-term,
iDescriptor, iNiR, plasma-bigscreen, plasma-desktop, shell) pod kątem tego, co blokuje
uruchomienie analizatora na nich bez fałszywych alarmów. Grupy: Quickshell-owe shelle
(Ambxst/DankMaterialShell/iNiR/shell — 0 plików C++, cały "backend" to skompilowany
framework Quickshell + importy KDE), oraz KDE/Qt z realnym C++ (plasma-desktop: 341 plików
C++, plasma-bigscreen: 88, cool-retro-term: 7, iDescriptor: 16).

Status: zaimplementowane pozycje są oznaczone `[x]`.

## Tier 1 — bez tego większość projektów zalewa false positive'ami

- [x] **Konfigurowalne opaque "external types"** (`[external_types]`) — typy z
  skompilowanych modułów C++/QML, których analizator nie może zintrospektować
  (np. Quickshell `ShellRoot`, `PanelWindow`, `Scope`, KDE `PlasmoidItem`).
  Instancjonowanie ich nie zgłasza już `UnknownType`, a ich własne przypisania property
  nie są walidowane (bo nie znamy realnej listy property) — ale **dzieci są sprawdzane
  normalnie**, w przeciwieństwie do już istniejącego mechanizmu opaque dla aliasowanych
  importów (`Kirigami.Icon`), który przerywa całą podgałąź.
  Zaimplementowane w `src/config.rs` (`ExternalTypesSection`), `src/checker/mod.rs`
  (`CheckContext::opaque_type_names`, `check_child`), `src/cmd_check.rs`.

- [x] **Konfigurowalne dodatkowe root types dla BFS** (`[roots] extra_types`) — obok
  zahardkodowanego `Window`/`ApplicationWindow`/`Dialog` (`cmd_check.rs`), Quickshell używa
  `ShellRoot`/`Scope`/`PanelWindow`, Plasma używa `PlasmoidItem`. Bez tego całe pliki (np.
  `pragma Singleton`) nigdy nie trafiają do `usage_paths` i ich zawartość nigdy się nie
  sprawdza (silent false negative, gorsze niż błąd — nic nie widać).
  Zaimplementowane w `src/config.rs` (`RootsSection`), `src/cmd_check.rs`.

- [x] **Generyczny skan metatypes z dowolnych modułów C++** — `qt_types_gen.rs` już
  parsuje standardowy format moc/AUTOMOC (`*_metatypes.json` z `QML.Element`,
  `QML.Foreign`, `QML.Attached`, properties/signals/methods), ale `load_all_classes()`
  (qt_types_gen.rs:107-122) ma zahardkodowaną listę modułów Qt (`qt6core`, `qt6gui`, ...)
  i szuka tylko w `lib/metatypes/`/`metatypes/`. KDE Frameworks, Plasma i Quickshell
  generują pliki w tym samym formacie przy budowaniu z CMake.
  Zaimplementowane jako `--extra-metatypes-dir <build_dir>` (repeatable) na
  `generate-qt-types`: rekurencyjnie zbiera wszystkie `*_metatypes.json` niezależnie od
  nazwy modułu (`find_all_metatypes_files`/`load_extra_classes` w `qt_types_gen.rs`) i
  scala je z bazą Qt (Qt wygrywa przy konflikcie nazw, potrzebne jako korzeń dziedziczenia).
  Zweryfikowane testem jednostkowym z syntetycznym modułem (bez realnego SDK Quickshella/
  KDE pod ręką) — potwierdzone poprawne rozwiązywanie `parent` przez scaloną bazę.
  **Uwaga**: trzeba wskazywać na *rozwiązane* metatypes (te które qmltyperegistrar emituje
  razem z `.qmltypes` modułu), nie na surowe pliki AUTOMOC z `CMakeFiles/*.dir/` — te mogą
  zawierać nierozwiązany placeholder `"auto"` w `QML.Element`, który generator świadomie
  pomija (tak jak już robi to dla samego Qt).

- [ ] **Skaner rejestracji C++ → auto-generowanie `[cpp_objects]`** — ręczne wypisywanie
  w TOML każdej klasy C++ dostępnej w QML jest nierealne przy setkach klas. `cpp_header.rs`
  już parsuje `Q_PROPERTY`/`signals:`/`Q_INVOKABLE` z pojedynczego headera — brakuje kroku
  wcześniejszego: przejścia po drzewie C++ i znalezienia `QML_ELEMENT`/`QML_SINGLETON`/
  `qmlRegisterType<T>()`/`qmlRegisterSingletonType<T>()`, żeby samemu zbudować listę klas +
  ścieżek do headerów.

- [ ] **Wirtualne/aliasowane importy katalogowe** (`import qs.modules.bar` w Quickshell) —
  UWAGA: zweryfikowane jako mniej krytyczne niż wstępnie sądzono, bo `known_types` w
  `cmd_check.rs:182` jest już płaską listą WSZYSTKICH nazw plików .qml w `--path`,
  niezależną od importów — więc nieznane typy z lokalnych katalogów projektu i tak są
  "znane" nawet bez poprawnego resolvowania `qs.*`. Realny problem pojawia się tylko przy
  odwołaniach przez prefiks przestrzeni nazw (`Services.foo` gdzie `Services` to alias
  całego katalogu) — rzadszy przypadek, niższy priorytet niż wcześniej myślano.

- [ ] **Parsowanie `qmldir`** — obecnie zupełnie nieobsługiwane. Projekty KDE mają
  dziesiątki `qmldir` (iNiR: 77) deklarujących `module org.kde.xxx`, `plugin xxxplugin`
  oraz **singletony** (`singleton Foo 1.0 Foo.qml`). Potrzebne do poprawnego mapowania
  `import org.kde.private.kicker as Kicker` na realne typy.

## Tier 2 — poprawiają trafność, mniejszy nakład

- [ ] **Walidacja wartości enumów** (`Type.Member`) — `QtTypeDb.enums` już istnieje
  (`qt_types.rs:14`), ale checker świadomie to olewa (`checker/mod.rs:815,1547` — "Skip Qt
  types used as enum namespaces"). plasma-desktop ma 20×`Q_ENUM` — spory potencjał
  wykrywania typo w `SomeType.SomeEnumValue`. **Ryzyko**: aktywacja wcześniej wyłączonego
  checku może wprowadzić false positive na istniejących testach/projektach jeśli baza Qt
  nie ma kompletnej listy wartości enum — testować bardzo ostrożnie na całym `test/`.

- [ ] **`pragma Singleton` jako sygnał dla BFS** — parser ignoruje wszystkie linie
  `pragma` (`parser/core.rs:65`). W samym `shell` jest 287 użyć `pragma Singleton`.
  Singleton nie jest nigdy "instancjonowany" jako dziecko — tylko odwoływany przez
  property access — więc może nigdy nie trafić do `usage_paths`. Można częściowo obejść
  już zaimplementowanym `[roots] extra_types`, ale natywne wsparcie (auto-detekcja pliku
  z `pragma Singleton` jako roota, bez potrzeby ręcznej konfiguracji) byłoby czystsze.

- [ ] **`pragma ComponentBehavior: Bound`** — 870 użyć w projektach Quickshell. Zmienia
  semantykę widoczności `id` wewnątrz delegatów `Loader`/`Repeater` (bez tego trzeba
  `required property`, żeby przekazać dane do delegate). Nieobsłużone = potencjalny false
  negative w checku "undefined name" wewnątrz delegatów.

- [ ] **Q_GADGET (typy wartościowe)** — 7 wystąpień w plasma-desktop. Właściwości
  grupowane bez QObject (jak `anchors`) — inny mechanizm niż `Q_PROPERTY` na wskaźniku do
  klasy. Sprawdzić czy `cpp_header.rs` już to łapie przez pattern-matching.

## Tier 3 — dalej idące, mniejszy priorytet

- [ ] **Walidacja sygnatur metod/sygnałów** — liczba i typy argumentów `Q_INVOKABLE`, nie
  tylko istnienie nazwy.

- [ ] **Auto-detekcja context properties** — skan `setContextProperty("name", ...)` w
  `main.cpp`, żeby nie trzeba było ręcznie dopisywać np. `plasmoid` (wstrzykiwane przez
  Plasma shell w runtime, nigdzie w QML niezadeklarowane).

- [ ] **Wielościeżkowe wyszukiwanie modułów** (odpowiednik `QML2_IMPORT_PATH`) — dla repo
  jak plasma-desktop, gdzie moduły QML są rozbite po wielu katalogach (`imports/`,
  `containments/`, `applets/`).

## Trzecia runda — weryfikacja na plasma-bigscreen (case study)

Zobacz `temp/plasma-bigscreen/ANALYZER_NOTES.md` po pełny opis krok po kroku (nie kopiowane
tutaj, bo `temp/` jest w `.gitignore` — to notatka roboczą dla tego konkretnego projektu).
Skrót:

- [x] **`[roots] treat_unreachable_as_roots`** — nowa opcja configu. KDE Plasma (KPackage)
  ładuje pliki QML po konwencji ścieżki, nie przez mały, wyliczalny zbiór typów root jak
  Quickshell (`ShellRoot`/`Scope`). `suggest-config` na plasma-bigscreen wypisał **38 różnych
  typów bazowych** jako kandydatów do `[roots] extra_types` — niemożliwe do sensownego
  utrzymania jako lista. Ta flaga rozwiązuje to inaczej: każdy plik, do którego BFS i tak nie
  dotrze, staje się swoim własnym rootem (kaskadująco, wciąż dociera do referencji z niego).
  Domyślnie `false` (zero ryzyka dla istniejących projektów).

- [x] **Naprawiony bug: `check_root` nie respektował `[external_types]` dla własnego
  base_type pliku** — tylko `check_child` miał tę logikę. Plik z rootem typu zewnętrznego
  (np. `ActionButton { opacity: ... }` z `org.kde.breeze.components`) miał flagowane
  `opacity` jako `Assignment to unknown property`, mimo że `ActionButton` był w
  `[external_types]`. Naprawione w `checker/mod.rs::check_root`.

- [x] **Naprawiony pre-existing bug: signal handlery nigdy nie respektowały "opaque"**
  — ani dla starego mechanizmu aliasowanych importów (`Kirigami.X { onXxx: ... }`), ani dla
  nowego `[external_types]`. `check_function` walidował istnienie sygnału bezwarunkowo, więc
  KAŻDY handler na opaque elemencie był gwarantowanym false positive. Naprawione: pomijamy
  walidację gdy `qt_props`/`qt_signals`/`declared_signals` są wszystkie puste (co — skoro
  każdy rozwiązywalny typ Qt ma przynajmniej `objectName` z `QtObject` — zdarza się tylko dla
  faktycznie nierozwiązanego/opaque typu). Na plasma-bigscreen to zredukowało błędy
  `Signal handler ... has no corresponding signal or property` z **52 do 7**.

- **Wynik końcowy na plasma-bigscreen**: 99 błędów / 93 z 106 plików nieprzeskanowanych
  (baseline) → 290 błędów / 0 nieprzeskanowanych (po configu + poprawkach). Błędy poszły W GÓRĘ
  bo dużo więcej projektu jest faktycznie sprawdzane — 93 nieprzeskanowane pliki to 93 pliki
  realnych błędów, które wcześniej były całkowicie niewidoczne.

- **Nieodkryty wcześniej gap, wciąż otwarty**: `qt_types_gen.rs`'s hardkodowana lista modułów
  Qt (`MODULES` w `load_all_classes`) nie zawiera np. `qtquickeffects` (`MultiEffect`,
  `DropShadow`), `qtquickshapes`, `qtlabsfolderlistmodel`, `qtqmlmodels`'s `DelegateChooser`
  itp. — to prawdziwe typy Qt, tylko nieujęte w generatorze, więc muszą iść do
  `[external_types]` (opaque) zamiast dostać pełną walidację. Osobna, mniejsza sprawa niż
  cały ten dokument, ale warto naprawić przy następnej rundzie (dopisać moduły do listy).

## Dodatkowo zaimplementowane (druga runda)

- [x] **`extends` w configu** — warstwowanie presetów (`config.rs::load_config_file` +
  `merge_configs`). Listy (`ignore.paths`, `globals.names`, `external_types.names`,
  `roots.extra_types`) są unią po całym łańcuchu; mapy (`new_child`, `cpp_objects`)
  scalane per-klucz, węższy config wygrywa przy konflikcie. Cykle wykrywane przez
  kanonikalizację ścieżek i przerywane z ostrzeżeniem. Preset dla Quickshella w
  `presets/quickshell.toml` (zweryfikowany end-to-end na `temp/shell`).

- [x] **`suggest-config` subcommand** — heurystyczny bootstrapping bez kompilacji.
  Uruchamia ten sam pipeline co `check`, ale zamiast błędów zbiera: (a) nazwy typów
  użytych jako dziecko, których nie ma ani w bazie Qt, ani wśród plików projektu →
  kandydaci do `[external_types]`; (b) typy bazowe plików nieosiągalnych z żadnego roota →
  kandydaci do `[roots] extra_types`. **Realny efekt uboczny odkryty przy testowaniu na
  `temp/shell`**: znaczna część "kandydatów" to nie prawdziwe typy zewnętrzne, a lokalne
  deklaracje `component Foo: Bar { ... }` w tym samym pliku — `known_types` budowane jest
  tylko z nazw plików `.qml`, więc inline `component` nigdy nie trafia do tej listy. To
  jest osobny, nieopisany wcześniej gap (nie naprawiony w tej rundzie) — `suggest-config`
  i tak jest użyteczny, ale trzeba uwzględnić ten szum przy przeglądzie listy.

## Zaimplementowane — jak używać

```toml
extends = "presets/quickshell.toml"   # opcjonalnie — warstwowanie na bazie presetu

[external_types]
# Typy z modułów C++/QML, których analizator nie może zintrospektować.
# (merged z listą presetu, jeśli extends jest ustawione)
names = ["MyProjectSpecificType"]

[roots]
# Dodatkowe typy root (obok wbudowanych Window/ApplicationWindow/Dialog),
# od których zaczyna się BFS wykrywania osiągalności plików.
extra_types = ["ShellRoot", "Scope"]
```

```bash
# Bootstrapping configu dla nowego projektu (bez kompilacji):
qml_static_analyzer suggest-config --path . --builtin-qt-version 6.8.3

# Pełna introspekcja C++ (wymaga zbudowanego projektu docelowego):
qml_static_analyzer generate-qt-types --qt-path ~/Qt/6.8.3/gcc_64 \
    --extra-metatypes-dir ~/quickshell/build --output qt_types_6.8.3_quickshell.json
```

Weryfikacja bezpieczeństwa zmian (obie rundy): `just test` (98 testów, 1 fail pre-existing
niezależny od tych zmian — potwierdzone przez `git stash` + porównanie na czystym
`master`), `cargo clippy --all-targets` (brak nowych ostrzeżeń w zmienionych plikach),
manualny test na fixture'ze w stylu Quickshell (`ShellRoot { Rectangle { unknownProp: 42 } }`),
test jednostkowy dla `extends`/cykli w `config.rs`, test jednostkowy dla scalania metatypes
w `qt_types_gen.rs`, oraz end-to-end na prawdziwym projekcie (`temp/shell` z presetem
Quickshella): BFS poprawnie dociera głęboko w drzewo przez `ShellRoot`/`Scope`, a te typy
(i pozostałe z presetu) nigdzie nie pojawiają się jako `Unknown type`.
