@echo off
setlocal EnableExtensions DisableDelayedExpansion
rem ============================================================================
rem  Compilation et deploiement de statusline.exe
rem
rem  Recompile le binaire de la ligne de statut depuis statusline-rs\, rafraichit
rem  la copie versionnee posee ici meme, puis la deploie vers le poste. C'est le
rem  chemin a prendre apres toute modification des sources Rust.
rem
rem  Trois emplacements, dans cet ordre :
rem    1. sortie de compilation   %LOCALAPPDATA%\statusline-rs\target\release\
rem       (detournee hors du lecteur Google Drive par statusline-rs\.cargo\
rem       config.toml ; le chemin exact est demande a cargo, jamais devine)
rem    2. copie versionnee        .claude\user-config\statusline.exe
rem       (la reference du projet : c'est elle que la restauration depose sur
rem       une machine sans Rust, d'ou son versionnement malgre son poids)
rem    3. binaire du poste        %USERPROFILE%\.claude\statusline.exe
rem       (celui que Claude Code lance a chaque rafraichissement)
rem
rem  Perimetre : le binaire, et lui seul. La synchronisation du reste de la
rem  configuration - settings.json, statusline.ps1, memory\, plans\ - reste
rem  l'affaire de sync-user-config.ps1, qui embarque d'ailleurs les memes trois
rem  etapes pour le binaire. Ce lanceur existe pour recompiler sans rien
rem  toucher d'autre, la ou l'autre script deplace toute la configuration.
rem
rem  Usage :
rem    compiler-statusline.bat          Compile, rafraichit, deploie
rem    compiler-statusline.bat /test    Idem, precede des tests unitaires et du
rem                                     harnais de non-regression (128 cas) ;
rem                                     rien n'est depose si un cas diverge
rem    compiler-statusline.bat /check   Compile et rapporte les ecarts, sans
rem                                     rien deposer
rem    compiler-statusline.bat /?       Cette aide
rem
rem  Codes de sortie : 0 succes ; 1 erreur ; 2 ecarts constates en /check ;
rem  4 aucun PowerShell trouve ; 5 crate introuvable ; 6 cargo absent ;
rem  7 compilation en echec ; 8 non-regression en echec.
rem ============================================================================

rem Sauvegarde de la page de code active pour restauration a la sortie.
for /f "tokens=2 delims=:" %%A in ('chcp 2^>nul') do set "OLD_CHCP=%%A"
if defined OLD_CHCP set "OLD_CHCP=%OLD_CHCP: =%"

rem Console en UTF-8 (65001) : les scripts appeles affichent des accents.
chcp 65001 >nul

rem Emplacement de ce fichier, le dossier user-config\, capture AVANT la boucle
rem d'arguments : shift decale aussi %0, apres quoi %~dp0 designerait l'argument
rem consomme et non ce script. Tout le reste passe par %ICI%.
set "ICI=%~dp0"

set "RC=0"
set "DEPOSER=1"
set "VERIFIER=0"

:args
if "%~1"=="" goto args_fin
if /i "%~1"=="/check" goto arg_check
if /i "%~1"=="/test" goto arg_test
if /i "%~1"=="/?" goto aide
if /i "%~1"=="-h" goto aide
if /i "%~1"=="--help" goto aide
echo.
echo [ERREUR] Argument inconnu : %~1
echo          Arguments acceptes : /test, /check, /?
set "RC=1"
goto fin

:arg_check
set "DEPOSER=0"
shift
goto args

:arg_test
set "VERIFIER=1"
shift
goto args

:args_fin

rem --- Chemins.
set "CRATE=%ICI%statusline-rs"
set "VERSIONNE=%ICI%statusline.exe"
set "ORACLE=%ICI%statusline.ps1"
set "HARNAIS=%ICI%test-statusline.ps1"

rem Cible : CLAUDE_CONFIG_DIR s'il est defini, sinon le profil.
set "CIBLE_DIR=%CLAUDE_CONFIG_DIR%"
if not defined CIBLE_DIR set "CIBLE_DIR=%USERPROFILE%\.claude"
set "CIBLE=%CIBLE_DIR%\statusline.exe"

if not exist "%CRATE%\Cargo.toml" goto crate_absent

rem --- Outils : cargo obligatoire ici, contrairement a sync-user-config.ps1 qui
rem     sait se rabattre sur la copie versionnee. Compiler est tout l'objet de
rem     ce script : sans chaine Rust, il n'a rien a faire.
where cargo.exe >nul 2>&1
if errorlevel 1 goto pas_de_cargo

rem PowerShell sert a deux choses : lire le dossier de sortie dans la sortie
rem JSON de cargo, et lancer le harnais sous /test.
set "PS="
where pwsh.exe >nul 2>&1
if not errorlevel 1 set "PS=pwsh.exe"
if not defined PS (
    where powershell.exe >nul 2>&1
    if not errorlevel 1 set "PS=powershell.exe"
)
if not defined PS goto pas_de_powershell

echo.
echo ============================================================================
echo  Compilation de la ligne de statut
echo ============================================================================
echo  Crate          : %CRATE%
echo  Copie projet   : %VERSIONNE%
echo  Binaire poste  : %CIBLE%
echo.

rem ---------------------------------------------------------------------------
rem  Compilation
rem
rem  Toutes les invocations de cargo se font depuis le dossier du crate, et
rem  --manifest-path ne dispenserait pas de s'y placer : cargo lit
rem  .cargo\config.toml en remontant depuis le REPERTOIRE COURANT, jamais depuis
rem  le manifeste. Lance d'ailleurs, il ecrirait dans le target\ par defaut, sur
rem  le lecteur reseau, et le binaire serait ensuite cherche la ou il n'est pas.
rem ---------------------------------------------------------------------------
pushd "%CRATE%" || goto pushd_impossible

rem Dossier de sortie demande a cargo plutot que devine : le detournement peut
rem changer dans .cargo\config.toml sans que ce script ait a le savoir.
rem
rem La commande passee a PowerShell n'emploie ni pipe ni redirection : dans un
rem "for /f" a guillemets inverses, cmd ne consomme pas les "^" d'echappement
rem et les transmettrait tels quels a cargo. D'ou le ConvertFrom-Json en position
rem prefixe, et l'appel a PowerShell sans guillemets - son nom n'a pas d'espace.
set "TARGET="
for /f "usebackq delims=" %%A in (`%PS% -NoProfile -ExecutionPolicy Bypass -Command "$ErrorActionPreference='SilentlyContinue'; (ConvertFrom-Json ((cargo metadata --format-version 1 --no-deps --offline) -join '')).target_directory"`) do set "TARGET=%%A"
if not defined TARGET goto target_inconnu
set "TARGET=%TARGET:/=\%"
set "COMPILE=%TARGET%\release\statusline.exe"

echo --- Compilation ------------------------------------------------------------
echo  Sortie         : %COMPILE%
echo.

rem --offline : les dependances sont deja dans le cache cargo du poste, et rien
rem ne doit etre telecharge pour recompiler la ligne de statut.
cargo build --release --offline
if errorlevel 1 goto echec_compilation

if not exist "%COMPILE%" goto binaire_introuvable

if "%VERIFIER%"=="0" goto compilation_fin

rem ---------------------------------------------------------------------------
rem  Verification, sous /test seulement
rem
rem  Deux filets successifs : les tests unitaires du crate, puis le harnais qui
rem  compare le binaire fraichement compile a statusline.ps1, l'oracle du
rem  portage - ligne ecrite, code de sortie et etat du cache. Le harnais
rem  detourne lui-meme %LOCALAPPDATA%, le cache reel de rythme n'est donc ni lu
rem  ni ecrit. -IgnorerCouleur depuis la capsule (19/09/2026, Q2 du chantier) :
rem  l'oracle est gele, seuls les cas NO_COLOR lui sont compares ; les cas
rem  colores sont couverts par les tests du crate.
rem ---------------------------------------------------------------------------
echo.
echo --- Tests unitaires --------------------------------------------------------
cargo test --offline
if errorlevel 1 goto echec_tests

echo.
echo --- Non-regression contre statusline.ps1 -----------------------------------
if not exist "%HARNAIS%" goto harnais_absent
if not exist "%ORACLE%" goto oracle_absent
"%PS%" -NoProfile -ExecutionPolicy Bypass -File "%HARNAIS%" -Candidat "%COMPILE%" -Reference "%ORACLE%" -IgnorerCouleur
if errorlevel 1 goto echec_regression

:compilation_fin
popd

rem ---------------------------------------------------------------------------
rem  Depot
rem ---------------------------------------------------------------------------
echo.
echo --- Depot ------------------------------------------------------------------

set "ECART=0"
set "DEPOSE=0"

rem --- 1. Copie versionnee du projet.
rem
rem     Recompiler les memes sources ne redonne pas toujours le meme fichier :
rem     Rust embarque le chemin des sources dans les emplacements de panic, et
rem     l'ordre de compilation joue. Une difference d'octets ne signale donc pas
rem     une difference de comportement - c'est le harnais qui en juge, pas fc.
set "MAJ_PROJET=0"
if not exist "%VERSIONNE%" (
    set "MAJ_PROJET=1"
) else (
    fc /b "%COMPILE%" "%VERSIONNE%" >nul 2>&1
    if errorlevel 1 set "MAJ_PROJET=1"
)

if "%MAJ_PROJET%"=="0" (
    echo   [=] Copie versionnee deja a jour.
) else if "%DEPOSER%"=="0" (
    echo   [!] Copie versionnee a rafraichir.
    set "ECART=1"
) else (
    copy /y "%COMPILE%" "%VERSIONNE%" >nul
    if errorlevel 1 goto echec_copie_projet
    set "DEPOSE=1"
    echo   [OK] Copie versionnee rafraichie.
)

rem --- 2. Binaire du poste.
set "MAJ_POSTE=0"
if not exist "%CIBLE%" (
    set "MAJ_POSTE=1"
) else (
    fc /b "%VERSIONNE%" "%CIBLE%" >nul 2>&1
    if errorlevel 1 set "MAJ_POSTE=1"
)

if "%MAJ_POSTE%"=="0" (
    echo   [=] Binaire du poste deja a jour.
    goto depot_fin
)
if "%DEPOSER%"=="0" (
    echo   [!] Binaire du poste a deployer.
    set "ECART=1"
    goto depot_fin
)

rem Un executable en cours d'execution ne peut pas etre ecrase, mais il peut
rem etre renomme. D'ou le detour par .old : la ligne de statut se relance toutes
rem les secondes, la collision n'a rien de theorique. Le .old residuel, si sa
rem suppression echoue parce que l'ancien binaire tourne encore, part au
rem deploiement suivant - sync-user-config.ps1 le declare ignorable a
rem l'inventaire pour cette raison.
if not exist "%CIBLE_DIR%" mkdir "%CIBLE_DIR%"
copy /y "%VERSIONNE%" "%CIBLE%.new" >nul
if errorlevel 1 goto echec_copie_poste
if exist "%CIBLE%.old" del /f /q "%CIBLE%.old" >nul 2>&1
if exist "%CIBLE%" move /y "%CIBLE%" "%CIBLE%.old" >nul
move /y "%CIBLE%.new" "%CIBLE%" >nul
if errorlevel 1 goto echec_bascule
if exist "%CIBLE%.old" del /f /q "%CIBLE%.old" >nul 2>&1
set "DEPOSE=1"
echo   [OK] Binaire deploye sur le poste.

:depot_fin
echo.
if "%DEPOSER%"=="0" (
    if "%ECART%"=="1" goto ecarts
    echo [OK] Rien a deposer : les trois copies concordent.
    set "RC=0"
    goto fin
)
if "%DEPOSE%"=="0" (
    echo [OK] Termine. Les trois copies concordaient deja : rien n'a ete ecrit.
    set "RC=0"
    goto fin
)
echo [OK] Termine. La ligne de statut sert le nouveau binaire des le prochain
echo      rafraichissement, sans redemarrer Claude Code.
if "%VERIFIER%"=="0" (
    echo.
    echo Rappel : /test verifie la non-regression contre statusline.ps1 avant de
    echo deposer quoi que ce soit.
)
set "RC=0"
goto fin

rem ---------------------------------------------------------------------------
rem  Sorties en erreur
rem ---------------------------------------------------------------------------

:aide
echo.
echo Usage :
echo   compiler-statusline.bat          Compile, rafraichit la copie du projet,
echo                                    deploie sur le poste
echo   compiler-statusline.bat /test    Idem, precede des tests unitaires et du
echo                                    harnais de non-regression
echo   compiler-statusline.bat /check   Compile et rapporte les ecarts, sans
echo                                    rien deposer
echo.
echo Sens du transfert : SOURCES -^> PROJET -^> POSTE, toujours.
set "RC=0"
goto fin

:crate_absent
echo.
echo [ERREUR] Crate introuvable : %CRATE%\Cargo.toml
echo          Ce fichier doit rester a la racine de user-config\, a cote du
echo          dossier statusline-rs\.
set "RC=5"
goto fin

:pas_de_cargo
echo.
echo [ERREUR] cargo.exe est absent du PATH : rien a compiler.
echo          Installez la chaine Rust ^(https://rustup.rs^), ou deployez la
echo          copie deja versionnee avec :
echo            pwsh -File "%ICI%sync-user-config.ps1" -Direction ProjectToGlobal
set "RC=6"
goto fin

:pas_de_powershell
echo.
echo [ERREUR] Ni pwsh.exe ni powershell.exe n'ont ete trouves dans le PATH.
echo          Installez PowerShell 7 ^(winget install Microsoft.PowerShell^).
set "RC=4"
goto fin

:pushd_impossible
echo.
echo [ERREUR] Impossible de se placer dans %CRATE%
set "RC=1"
goto fin

:target_inconnu
popd
echo.
echo [ERREUR] cargo metadata n'a pas rendu de dossier de sortie. Le crate est
echo          peut-etre invalide, ou une dependance manque au cache local
echo          ^(--offline^).
set "RC=7"
goto fin

:echec_compilation
popd
echo.
echo [ERREUR] La compilation a echoue. Rien n'a ete depose : la copie versionnee
echo          et le binaire du poste restent ceux d'avant.
set "RC=7"
goto fin

:binaire_introuvable
popd
echo.
echo [ERREUR] Compilation annoncee reussie, mais %COMPILE% est absent.
set "RC=7"
goto fin

:echec_tests
popd
echo.
echo [ERREUR] Les tests unitaires du crate echouent. Rien n'a ete depose.
set "RC=8"
goto fin

:harnais_absent
popd
echo.
echo [ERREUR] Harnais introuvable : %HARNAIS%
echo          Relancez sans /test pour deposer sans verifier.
set "RC=8"
goto fin

:oracle_absent
popd
echo.
echo [ERREUR] Oracle introuvable : %ORACLE%
echo          La non-regression se mesure contre le script d'origine ; sans lui,
echo          relancez sans /test.
set "RC=8"
goto fin

:echec_regression
popd
echo.
echo [ERREUR] Le binaire compile diverge de statusline.ps1 sur au moins un cas.
echo          Rien n'a ete depose. Le detail des divergences est ci-dessus.
echo.
echo          Une seule reserve avant de conclure a une regression : les cas
echo          "rythme epuisement" projettent une heure a partir de l'instant
echo          present, et le harnais lance les deux implementations l'une apres
echo          l'autre. Une divergence d'UNE MINUTE sur cette seule heure est une
echo          course, pas un ecart de comportement - relancez pour trancher.
set "RC=8"
goto fin

:echec_copie_projet
echo.
echo [ERREUR] Copie vers %VERSIONNE% impossible.
set "RC=1"
goto fin

:echec_copie_poste
echo.
echo [ERREUR] Copie vers %CIBLE%.new impossible.
set "RC=1"
goto fin

:echec_bascule
echo.
echo [ERREUR] Bascule de %CIBLE%.new vers %CIBLE% impossible. L'ancien binaire
echo          est peut-etre reste sous %CIBLE%.old : verifiez le dossier.
set "RC=1"
goto fin

:ecarts
echo [!] Ecarts constates. Relancez sans /check pour les resorber.
set "RC=2"
goto fin

rem ---------------------------------------------------------------------------
rem  Sortie
rem ---------------------------------------------------------------------------
:fin
setlocal EnableDelayedExpansion

rem Pause uniquement si la fenetre a ete ouverte par double-clic (cmd /c ...),
rem pour qu'un appel depuis un terminal rende la main immediatement.
set "SHOULD_PAUSE=0"
echo !cmdcmdline! | findstr /i /c:" /c" >nul && set "SHOULD_PAUSE=1"
echo !cmdcmdline! | findstr /i /c:" /k" >nul && set "SHOULD_PAUSE=0"
echo !cmdcmdline! | findstr /i "compiler-statusline.bat" >nul && set "SHOULD_PAUSE=1"
if "!SHOULD_PAUSE!"=="1" (
    echo.
    echo Appuyez sur une touche pour fermer cette fenetre...
    pause >nul
)
endlocal

rem Restauration de la page de code d'origine de la console.
if defined OLD_CHCP chcp %OLD_CHCP% >nul
exit /b %RC%
