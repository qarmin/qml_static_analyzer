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

- [ ] **Generyczny skan metatypes z dowolnych modułów C++** — `qt_types_gen.rs` już
  parsuje standardowy format moc/AUTOMOC (`*_metatypes.json` z `QML.Element`,
  `QML.Foreign`, `QML.Attached`, properties/signals/methods), ale `load_all_classes()`
  (qt_types_gen.rs:107-122) ma zahardkodowaną listę modułów Qt (`qt6core`, `qt6gui`, ...)
  i szuka tylko w `lib/metatypes/`/`metatypes/`. KDE Frameworks, Plasma i Quickshell
  generują pliki w tym samym formacie przy budowaniu z CMake. Trzeba: nowy subcommand
  `generate-module-types --scan-dir <build_dir>` który rekurencyjnie zbiera wszystkie
  `*_metatypes.json` niezależnie od nazwy modułu i scala je z istniejącą bazą Qt.
  Rozwiązuje to problem klas C++ dla plasma-desktop/plasma-bigscreen i (po zbudowaniu ze
  źródeł) samego Quickshella.

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

## Zaimplementowane — jak używać

```toml
[external_types]
# Typy z modułów C++/QML, których analizator nie może zintrospektować.
names = ["ShellRoot", "PanelWindow", "Scope", "PlasmoidItem"]

[roots]
# Dodatkowe typy root (obok wbudowanych Window/ApplicationWindow/Dialog),
# od których zaczyna się BFS wykrywania osiągalności plików.
extra_types = ["ShellRoot", "Scope"]
```

Weryfikacja bezpieczeństwa zmian: `just test` (98 testów, 1 fail pre-existing niezależny
od tych zmian — potwierdzone przez `git stash` + porównanie na czystym `master`), `cargo
clippy --all-targets` (brak nowych ostrzeżeń w zmienionych plikach), oraz manualny test na
fixture'ze w stylu Quickshell (`ShellRoot { Rectangle { unknownProp: 42 } }`):
- bez configu: plik nieosiągalny, 0 błędów (zachowanie identyczne jak przed zmianą),
- z configem: `ShellRoot` nie jest już `UnknownType`, plik jest sprawdzany, a
  `unknownProp` na `Rectangle` w środku i tak zostaje poprawnie wykryte.
