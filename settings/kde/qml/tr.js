.pragma library

// Translations of the settings window, and the choice of the display language.
//
// The key is the Korean source text (like a gettext msgid). The table between
// the BEGIN and END lines is plain JSON, so tests/check_translations.py can
// read it.
//
// DRAFT: the English, Japanese and Chinese texts were written without review
// by a native speaker, which DESIGN.md (4.1) asks for. Say so in the window
// (isDraft) and do not call them reviewed until someone has.
//
// The language follows the system language. For trying another one, pass
// --ui-language=ko|en|ja|zh-Hant|zh-Hans to the program, or set LANGUAGE.

var languages = ["ko", "en", "ja", "zh-Hant", "zh-Hans"]

// BEGIN TABLE
var table = {
  "(없음)": {
    "en": "(none)",
    "ja": "（なし）",
    "zh-Hant": "（無）",
    "zh-Hans": "（无）"
  },
  "Backspace로 지우는 단위:": {
    "en": "What Backspace deletes:",
    "ja": "Backspace で消す単位:",
    "zh-Hant": "Backspace 刪除的單位：",
    "zh-Hans": "Backspace 删除的单位："
  },
  "KDE의 가상 키보드 설정을 바꿉니다.\n파일: %1\n지금 선택된 입력기: %2 → easime\n바꾼 뒤 다시 로그인하면 적용되며, 이 화면에서 언제든 되돌릴 수 있습니다.": {
    "en": "This changes KDE's virtual keyboard setting.\nFile: %1\nSelected input method now: %2 → easime\nIt takes effect after you log in again, and you can undo it here at any time.",
    "ja": "KDE の仮想キーボードの設定を変更します。\nファイル: %1\n現在選ばれている入力メソッド: %2 → easime\n再ログインすると反映されます。この画面でいつでも元に戻せます。",
    "zh-Hant": "將變更 KDE 的虛擬鍵盤設定。\n檔案：%1\n目前選用的輸入法：%2 → easime\n重新登入後生效，您隨時可在此畫面還原。",
    "zh-Hans": "将更改 KDE 的虚拟键盘设置。\n文件：%1\n当前选用的输入法：%2 → easime\n重新登录后生效，您随时可以在此界面还原。"
  },
  "easime 설정": {
    "en": "easime Settings",
    "ja": "easime 設定",
    "zh-Hant": "easime 設定",
    "zh-Hans": "easime 设置"
  },
  "easime 설정을 기본값으로 되돌리기": {
    "en": "Reset easime settings to defaults",
    "ja": "easime の設定を初期値に戻す",
    "zh-Hant": "將 easime 設定還原為預設值",
    "zh-Hans": "将 easime 设置恢复为默认值"
  },
  "easime가 아직 입력기로 선택되지 않았습니다. 선택하면 다음에 로그인할 때부터 easime로 입력합니다.": {
    "en": "easime is not selected as your input method yet. Once you select it, easime types from your next login.",
    "ja": "easime はまだ入力メソッドとして選ばれていません。選ぶと、次回のログインから easime で入力できます。",
    "zh-Hant": "尚未選用 easime 作為輸入法。選用後，下次登入起即可使用 easime 輸入。",
    "zh-Hans": "尚未选用 easime 作为输入法。选用后，下次登录起即可使用 easime 输入。"
  },
  "easime가 입력기로 선택되어 실행 중입니다. 글을 쓰는 곳에서 입력해 보세요. 입력되지 않으면 로그 파일을 확인하세요.": {
    "en": "easime is selected as the input method and running. Try typing where you write. If nothing is typed, check the log file.",
    "ja": "easime が入力メソッドとして選ばれ、動作中です。文章を書く場所で入力してみてください。入力できない場合はログファイルを確認してください。",
    "zh-Hant": "easime 已選為輸入法並正在執行。請在要寫字的地方試著輸入。若無法輸入，請查看記錄檔。",
    "zh-Hans": "easime 已选为输入法并正在运行。请在要写字的地方试着输入。若无法输入，请查看日志文件。"
  },
  "easime를 입력기로 사용할까요?": {
    "en": "Use easime as your input method?",
    "ja": "easime を入力メソッドとして使いますか？",
    "zh-Hant": "要將 easime 設為輸入法嗎？",
    "zh-Hans": "要将 easime 设为输入法吗？"
  },
  "easime를 입력기로 선택했습니다. 다시 로그인하면 사용할 수 있습니다.": {
    "en": "easime is selected as your input method. You can use it after you log in again.",
    "ja": "easime を入力メソッドとして選びました。再ログインすると使えます。",
    "zh-Hant": "已選用 easime 作為輸入法。重新登入後即可使用。",
    "zh-Hans": "已选用 easime 作为输入法。重新登录后即可使用。"
  },
  "easime의 설정이 모두 기본값으로 바뀝니다. 지금 설정 파일은 config.toml.bak으로 보관되며, 입력기 선택이나 다른 프로그램의 설정은 바뀌지 않습니다.": {
    "en": "All of easime's settings will be reset to their defaults. The current settings file is kept as config.toml.bak. Your input method selection and other programs' settings are not changed.",
    "ja": "easime の設定がすべて初期値に戻ります。現在の設定ファイルは config.toml.bak として保管され、入力メソッドの選択や他のプログラムの設定は変わりません。",
    "zh-Hant": "easime 的所有設定都會還原為預設值。目前的設定檔會保留為 config.toml.bak，輸入法的選擇和其他程式的設定不會改變。",
    "zh-Hans": "easime 的所有设置都会恢复为默认值。当前的设置文件会保留为 config.toml.bak，输入法的选择和其他程序的设置不会改变。"
  },
  "easime의 입력기 파일(easime.desktop)을 찾지 못해 입력기로 선택할 수 없습니다. easime가 제대로 설치되었는지 확인해 주세요.": {
    "en": "easime's input method file (easime.desktop) was not found, so it cannot be selected. Please check that easime is installed correctly.",
    "ja": "easime の入力メソッドファイル（easime.desktop）が見つからないため選べません。easime が正しくインストールされているか確認してください。",
    "zh-Hant": "找不到 easime 的輸入法檔案（easime.desktop），因此無法選用。請確認 easime 已正確安裝。",
    "zh-Hans": "找不到 easime 的输入法文件（easime.desktop），因此无法选用。请确认 easime 已正确安装。"
  },
  "中文:": {
    "en": "中文:",
    "ja": "中文:",
    "zh-Hant": "中文:",
    "zh-Hans": "中文:"
  },
  "日本語:": {
    "en": "日本語:",
    "ja": "日本語:",
    "zh-Hant": "日本語:",
    "zh-Hans": "日本語:"
  },
  "가상 키보드 설정을 easime를 고르기 전의 값(%1)으로 되돌립니다. 다시 로그인하면 적용됩니다.": {
    "en": "The virtual keyboard setting goes back to its value before easime was chosen (%1). It takes effect after you log in again.",
    "ja": "仮想キーボードの設定を、easime を選ぶ前の値（%1）に戻します。再ログインすると反映されます。",
    "zh-Hant": "虛擬鍵盤設定將還原為選用 easime 之前的值（%1）。重新登入後生效。",
    "zh-Hans": "虚拟键盘设置将还原为选用 easime 之前的值（%1）。重新登录后生效。"
  },
  "그 사이 다른 입력기가 선택되어 있어 되돌리지 않았습니다.": {
    "en": "Another input method was selected in the meantime, so nothing was changed back.",
    "ja": "その間に別の入力メソッドが選ばれていたため、元には戻しませんでした。",
    "zh-Hant": "期間已選用其他輸入法，因此未還原。",
    "zh-Hans": "期间已选用其他输入法，因此未还原。"
  },
  "글자 단위 (한 → 지움)": {
    "en": "Whole syllable (한 → erased)",
    "ja": "音節単位（한 → 消去）",
    "zh-Hant": "以字為單位（한 → 刪除）",
    "zh-Hans": "以字为单位（한 → 删除）"
  },
  "글자를 조합하는 중에만 적용됩니다. 이미 입력을 마친 글자는 앱의 Backspace 동작을 따릅니다.": {
    "en": "Applies only while a syllable is being composed. Characters you have already entered follow the application's own Backspace behavior.",
    "ja": "文字を組み立てている間だけ適用されます。入力が終わった文字は、アプリの Backspace の動作に従います。",
    "zh-Hant": "只在組字過程中套用。已經輸入完成的字，會依應用程式本身的 Backspace 行為處理。",
    "zh-Hans": "仅在组字过程中生效。已经输入完成的字，会按应用本身的 Backspace 行为处理。"
  },
  "기본값으로 되돌렸습니다.": {
    "en": "Reset to defaults.",
    "ja": "初期値に戻しました。",
    "zh-Hant": "已還原為預設值。",
    "zh-Hans": "已恢复为默认值。"
  },
  "기본값으로 되돌렸습니다. 이전 파일은 %1에 보관했습니다.": {
    "en": "Reset to defaults. The previous file was kept at %1.",
    "ja": "初期値に戻しました。以前のファイルは %1 に保管しました。",
    "zh-Hant": "已還原為預設值。先前的檔案已保留在 %1。",
    "zh-Hans": "已恢复为默认值。之前的文件已保留在 %1。"
  },
  "기본값으로 되돌릴까요?": {
    "en": "Reset to defaults?",
    "ja": "初期値に戻しますか？",
    "zh-Hant": "要還原為預設值嗎？",
    "zh-Hans": "要恢复为默认值吗？"
  },
  "다시 읽기": {
    "en": "Reload",
    "ja": "再読み込み",
    "zh-Hant": "重新讀取",
    "zh-Hans": "重新读取"
  },
  "단축키": {
    "en": "Shortcuts",
    "ja": "ショートカット",
    "zh-Hant": "快速鍵",
    "zh-Hans": "快捷键"
  },
  "두벌식 입력 · 조합 중 한 글자 한자 변환(시험 중)": {
    "en": "Dubeolsik typing · one-syllable Hanja conversion while composing (testing)",
    "ja": "ドゥボルシク入力 · 入力中の1文字ハンジャ変換（試験中）",
    "zh-Hant": "두벌식（Dubeolsik）輸入 · 組字時單字漢字（Hanja）轉換（測試中）",
    "zh-Hans": "두벌식（Dubeolsik）输入 · 组字时单字汉字（Hanja）转换（测试中）"
  },
  "둘 다 (현재 기본값)": {
    "en": "Both (current default)",
    "ja": "両方（現在の初期値）",
    "zh-Hant": "兩者皆可（目前預設）",
    "zh-Hans": "两者皆可（当前默认）"
  },
  "로그 파일:": {
    "en": "Log file:",
    "ja": "ログファイル:",
    "zh-Hant": "記錄檔：",
    "zh-Hans": "日志文件："
  },
  "로마자 → 가나 입력 · 단어 단위 한자 변환(시험 중)": {
    "en": "Romaji to kana typing · word-by-word kanji conversion (testing)",
    "ja": "ローマ字 → かな入力 · 単語単位の漢字変換（試験中）",
    "zh-Hant": "羅馬字 → 假名輸入 · 逐詞漢字轉換（測試中）",
    "zh-Hans": "罗马字 → 假名输入 · 逐词汉字转换（测试中）"
  },
  "모양": {
    "en": "Appearance",
    "ja": "外観",
    "zh-Hant": "外觀",
    "zh-Hans": "外观"
  },
  "바꾼 내용은 바로 설정 파일에 저장되고, 실행 중인 easime가 곧 다시 읽습니다. 다시 시작할 필요는 없습니다.": {
    "en": "Changes are saved to the settings file right away, and the running easime will read them again shortly. There is no need to restart.",
    "ja": "変更はすぐに設定ファイルへ保存され、実行中の easime がまもなく読み直します。再起動は不要です。",
    "zh-Hant": "變更會立即儲存到設定檔，執行中的 easime 稍後會重新讀取。不需要重新啟動。",
    "zh-Hans": "更改会立即保存到设置文件，正在运行的 easime 稍后会重新读取。无需重启。"
  },
  "사전 파일 %1개를 모두 읽지 못해 일본어를 켜도 가나만 후보로 나옵니다. 로그 파일에서 이유를 확인하고 easime 패키지를 다시 설치해 주세요.": {
    "en": "None of the %1 dictionary file(s) could be read, so even with Japanese on only kana are offered. See the log file for why, and reinstall the easime package.",
    "ja": "辞書ファイル %1 個をすべて読み込めないため、日本語をオンにしても候補は仮名だけです。ログファイルで理由を確認し、easime パッケージを再インストールしてください。",
    "zh-Hant": "%1 個詞典檔案全部無法讀取，因此即使開啟日文，候選也只有假名。請在記錄檔查看原因，並重新安裝 easime 套件。",
    "zh-Hans": "%1 个词典文件全部无法读取，因此即使开启日文，候选也只有假名。请在日志文件中查看原因，并重新安装 easime 软件包。"
  },
  "상태 표시:": {
    "en": "Indicator:",
    "ja": "状態表示:",
    "zh-Hant": "狀態顯示：",
    "zh-Hans": "状态显示："
  },
  "설정 초기화:": {
    "en": "Reset settings:",
    "ja": "設定の初期化:",
    "zh-Hant": "重設設定：",
    "zh-Hans": "重置设置："
  },
  "설정 파일:": {
    "en": "Settings file:",
    "ja": "設定ファイル:",
    "zh-Hant": "設定檔：",
    "zh-Hans": "设置文件："
  },
  "설정 파일을 다시 읽었습니다.": {
    "en": "The settings file was reloaded.",
    "ja": "設定ファイルを再読み込みしました。",
    "zh-Hant": "已重新讀取設定檔。",
    "zh-Hans": "已重新读取设置文件。"
  },
  "설정 파일을 읽을 수 없어 저장하지 않았습니다.": {
    "en": "The settings file cannot be read, so nothing was saved.",
    "ja": "設定ファイルを読めないため、保存しませんでした。",
    "zh-Hant": "無法讀取設定檔，因此未儲存。",
    "zh-Hans": "无法读取设置文件，因此未保存。"
  },
  "설정 파일을 읽지 못해 기본값을 보여 드리고 있습니다. 파일을 직접 고친 뒤 '다시 읽기'를 누르거나, '일반'에서 '기본값으로 되돌리기'를 눌러 새로 시작하세요. 읽을 수 없는 동안에는 파일을 지우지 않도록 변경을 저장하지 않습니다.": {
    "en": "The settings file could not be read, so the defaults are shown. Fix the file by hand and press \"Reload\", or press \"Reset to defaults\" under \"General\" to start over. While the file cannot be read, changes are not saved, so the file is never erased.",
    "ja": "設定ファイルを読めないため、初期値を表示しています。ファイルを直接直してから「再読み込み」を押すか、「全般」の「初期値に戻す」で最初からやり直してください。読めない間は、ファイルを消してしまわないよう変更を保存しません。",
    "zh-Hant": "無法讀取設定檔，目前顯示的是預設值。請自行修正檔案後按「重新讀取」，或在「一般」中按「還原為預設值」重新開始。無法讀取檔案時，為避免檔案被清除，不會儲存變更。",
    "zh-Hans": "无法读取设置文件，当前显示的是默认值。请自行修改文件后点击“重新读取”，或在“常规”中点击“恢复为默认值”重新开始。无法读取文件时，为避免文件被清除，不会保存更改。"
  },
  "쓰는 언어만 켜 두세요. 어느 언어도 꼭 켜야 하는 것은 아닙니다. 전환 키를 누를 때마다 아래 순서대로 바뀌며, 열려 있는 모든 창에 똑같이 적용됩니다.": {
    "en": "Turn on only the languages you use. No language is required. Each time you press the switch key, input moves through the order below, and the same applies to every open window.",
    "ja": "使う言語だけをオンにしてください。どの言語も必ずオンにする必要はありません。切り替えキーを押すたびに下の順に切り替わり、開いているすべてのウィンドウに同じように適用されます。",
    "zh-Hant": "只需開啟您會使用的語言。不一定要開啟任何語言。每按一次切換鍵，就會依下列順序切換，並同樣套用到所有開啟的視窗。",
    "zh-Hans": "只需打开您要使用的语言。不一定要打开任何语言。每按一次切换键，就会按下列顺序切换，并同样应用到所有已打开的窗口。"
  },
  "아직 직접 고르지 않아 시스템 언어에 맞춰져 있습니다. 직접 바꾸면 그 선택이 저장됩니다.": {
    "en": "You have not chosen yet, so this follows your system language. Once you change it, your choice is kept.",
    "ja": "まだ選んでいないため、システムの言語に合わせています。変更すると、その選択が保存されます。",
    "zh-Hant": "尚未自行選擇，目前依系統語言設定。變更後會儲存您的選擇。",
    "zh-Hans": "尚未自行选择，目前按系统语言设置。更改后会保存您的选择。"
  },
  "알 수 없는 값이라 적용하지 않았습니다.": {
    "en": "Unknown value, so it was not applied.",
    "ja": "不明な値のため、適用しませんでした。",
    "zh-Hant": "未知的值，因此未套用。",
    "zh-Hans": "未知的值，因此未应用。"
  },
  "언어": {
    "en": "Languages",
    "ja": "言語",
    "zh-Hant": "語言",
    "zh-Hans": "语言"
  },
  "언어 전환 키:": {
    "en": "Language switch key:",
    "ja": "言語切り替えキー:",
    "zh-Hant": "語言切換鍵：",
    "zh-Hans": "语言切换键："
  },
  "언어 화면 열기": {
    "en": "Open the Languages page",
    "ja": "言語ページを開く",
    "zh-Hant": "開啟語言頁面",
    "zh-Hans": "打开语言页面"
  },
  "언어를 바꿀 때 커서 옆에 상자로 알려 주기": {
    "en": "Show a box next to the cursor when the language changes",
    "ja": "言語を切り替えるときにカーソルの横に四角で知らせる",
    "zh-Hant": "切換語言時，在游標旁以方塊提示",
    "zh-Hans": "切换语言时，在光标旁用方框提示"
  },
  "영어": {
    "en": "English",
    "ja": "英語",
    "zh-Hant": "英文",
    "zh-Hans": "英文"
  },
  "오른쪽 Alt": {
    "en": "Right Alt",
    "ja": "右 Alt",
    "zh-Hant": "右 Alt",
    "zh-Hans": "右 Alt"
  },
  "이 설정 창의 번역은 초안이며 원어민 검토를 거치지 않았습니다.": {
    "en": "This translation of the settings window is a draft and has not been reviewed by a native speaker.",
    "ja": "この設定ウィンドウの翻訳は草案で、ネイティブスピーカーによる確認を受けていません。",
    "zh-Hant": "此設定視窗的翻譯為草稿，尚未經過母語人士審閱。",
    "zh-Hans": "此设置窗口的翻译为草稿，尚未经过母语人士审阅。"
  },
  "이전 입력기로 되돌렸습니다. 다시 로그인하면 적용됩니다.": {
    "en": "Went back to the previous input method. It takes effect after you log in again.",
    "ja": "前の入力メソッドに戻しました。再ログインすると反映されます。",
    "zh-Hant": "已還原為先前的輸入法。重新登入後生效。",
    "zh-Hans": "已还原为之前的输入法。重新登录后生效。"
  },
  "이전 입력기로 되돌리기…": {
    "en": "Go back to the previous input method…",
    "ja": "前の入力メソッドに戻す…",
    "zh-Hant": "還原為先前的輸入法…",
    "zh-Hans": "还原为之前的输入法…"
  },
  "이전 입력기로 되돌릴까요?": {
    "en": "Go back to the previous input method?",
    "ja": "前の入力メソッドに戻しますか？",
    "zh-Hant": "要還原為先前的輸入法嗎？",
    "zh-Hans": "要还原为之前的输入法吗？"
  },
  "일반": {
    "en": "General",
    "ja": "全般",
    "zh-Hant": "一般",
    "zh-Hans": "常规"
  },
  "일본어 사전:": {
    "en": "Japanese dictionary:",
    "ja": "日本語辞書:",
    "zh-Hant": "日文詞典：",
    "zh-Hans": "日文词典："
  },
  "읽지 못한 사전 파일이 %1개 있습니다. 나머지로 변환하며, 자세한 이유는 로그 파일에 있습니다.": {
    "en": "%1 dictionary file(s) could not be read. Conversion uses the rest; the log file says why.",
    "ja": "読み込めない辞書ファイルが %1 個あります。残りで変換します。詳しい理由はログファイルにあります。",
    "zh-Hant": "有 %1 個詞典檔案無法讀取。將使用其餘檔案轉換，原因請見記錄檔。",
    "zh-Hans": "有 %1 个词典文件无法读取。将使用其余文件转换，原因请见日志文件。"
  },
  "입력기로 사용하기…": {
    "en": "Use as input method…",
    "ja": "入力メソッドとして使う…",
    "zh-Hant": "設為輸入法…",
    "zh-Hans": "设为输入法…"
  },
  "입력창을 누르거나 창을 옮길 때는 표시하지 않고, 사용자가 언어를 바꿀 때만 잠깐 나타납니다.": {
    "en": "It is not shown when you click an input field or move between windows; it appears briefly only when you switch the language.",
    "ja": "入力欄をクリックしたりウィンドウを移動したりしたときは表示せず、言語を切り替えたときだけ短く表示されます。",
    "zh-Hant": "點選輸入欄或切換視窗時不會顯示，只有在使用者切換語言時才會短暫出現。",
    "zh-Hans": "点击输入框或切换窗口时不会显示，只有在用户切换语言时才会短暂出现。"
  },
  "있음 (파일 %1개). 단어 단위로 한자를 고를 수 있습니다.": {
    "en": "Found (%1 files). You can pick kanji word by word.",
    "ja": "見つかりました（ファイル %1 個）。単語ごとに漢字を選べます。",
    "zh-Hant": "已找到（%1 個檔案）。可逐詞選擇漢字。",
    "zh-Hans": "已找到（%1 个文件）。可逐词选择汉字。"
  },
  "자모 단위 (한 → 하 → ㅎ)": {
    "en": "One jamo at a time (한 → 하 → ㅎ)",
    "ja": "字母（ジャモ）単位（한 → 하 → ㅎ）",
    "zh-Hant": "以字母（子音／母音）為單位（한 → 하 → ㅎ）",
    "zh-Hans": "以字母（辅音/元音）为单位（한 → 하 → ㅎ）"
  },
  "저장하지 못했습니다. 변경하기 전 값을 그대로 보여 드립니다.": {
    "en": "Could not save. The values from before the change are shown.",
    "ja": "保存できませんでした。変更前の値をそのまま表示しています。",
    "zh-Hant": "無法儲存。目前顯示的是變更前的值。",
    "zh-Hans": "无法保存。当前显示的是更改前的值。"
  },
  "저장했습니다. 실행 중인 easime에 곧 반영됩니다.": {
    "en": "Saved. The running easime will pick it up shortly.",
    "ja": "保存しました。実行中の easime にまもなく反映されます。",
    "zh-Hant": "已儲存。執行中的 easime 稍後就會套用。",
    "zh-Hans": "已保存。正在运行的 easime 稍后就会应用。"
  },
  "전환 순서: %1": {
    "en": "Switching order: %1",
    "ja": "切り替え順: %1",
    "zh-Hant": "切換順序：%1",
    "zh-Hans": "切换顺序：%1"
  },
  "준비 중": {
    "en": "Not available yet",
    "ja": "準備中",
    "zh-Hant": "準備中",
    "zh-Hans": "准备中"
  },
  "지금은 영어로만 입력됩니다. 한국어나 日本語로도 입력하려면 언어 화면에서 켜 주세요.": {
    "en": "easime currently types English only. To type Korean or Japanese too, turn them on in the Languages page.",
    "ja": "現在は英語のみ入力できます。韓国語や日本語も入力するには、「言語」ページでオンにしてください。",
    "zh-Hant": "目前只能輸入英文。若要同時輸入韓文或日文，請到「語言」頁面開啟。",
    "zh-Hans": "当前只能输入英文。若要同时输入韩文或日文，请到“语言”页面开启。"
  },
  "지원 상태": {
    "en": "Support status",
    "ja": "対応状況",
    "zh-Hant": "支援狀態",
    "zh-Hans": "支持状态"
  },
  "직접 고쳤다면:": {
    "en": "If you edited it by hand:",
    "ja": "直接編集した場合:",
    "zh-Hant": "若自行修改過：",
    "zh-Hans": "如果自行修改过："
  },
  "찾지 못했습니다. 일본어를 켜도 가나만 후보로 나옵니다. easime 패키지를 다시 설치하거나 사전 파일을 아래 폴더 중 한 곳에 두세요:\n%1": {
    "en": "Not found. Even with Japanese on, only kana are offered as candidates. Reinstall the easime package, or put a dictionary file in one of these folders:\n%1",
    "ja": "見つかりません。日本語をオンにしても候補は仮名だけです。easime パッケージを再インストールするか、辞書ファイルを次のいずれかのフォルダーに置いてください:\n%1",
    "zh-Hant": "找不到。即使開啟日文，候選也只有假名。請重新安裝 easime 套件，或將詞典檔案放在下列其中一個資料夾：\n%1",
    "zh-Hans": "找不到。即使开启日文，候选也只有假名。请重新安装 easime 软件包，或将词典文件放在下列文件夹之一：\n%1"
  },
  "처음부터 다시": {
    "en": "Start over",
    "ja": "最初からやり直す",
    "zh-Hant": "從頭開始",
    "zh-Hans": "从头开始"
  },
  "켜진 언어가 없어 영어로만 입력합니다.": {
    "en": "No language is turned on, so you can only type in English.",
    "ja": "オンの言語がないため、英語でのみ入力できます。",
    "zh-Hant": "沒有開啟任何語言，因此只能輸入英文。",
    "zh-Hans": "没有打开任何语言，因此只能输入英文。"
  },
  "폴더 열기": {
    "en": "Open folder",
    "ja": "フォルダーを開く",
    "zh-Hant": "開啟資料夾",
    "zh-Hans": "打开文件夹"
  },
  "표시 시간:": {
    "en": "Display time:",
    "ja": "表示時間:",
    "zh-Hant": "顯示時間：",
    "zh-Hans": "显示时间："
  },
  "한/영 키": {
    "en": "Hangul key (한/영)",
    "ja": "ハングルキー（한/영）",
    "zh-Hant": "韓/英鍵（한/영）",
    "zh-Hans": "韩/英键（한/영）"
  },
  "한/영 키는 누르는 즉시 전환합니다. 오른쪽 Alt는 다른 키를 누르지 않고 눌렀다 뗄 때만 전환하므로, Alt와 다른 키를 함께 누르는 단축키는 그대로 쓸 수 있습니다.": {
    "en": "The Hangul key switches as soon as you press it. Right Alt switches only when you press and release it without pressing another key, so shortcuts that combine Alt with another key keep working.",
    "ja": "ハングルキーは押した瞬間に切り替わります。右 Alt は、他のキーを押さずに押して離したときだけ切り替わるので、Alt と他のキーを組み合わせるショートカットはそのまま使えます。",
    "zh-Hant": "韓/英鍵在按下的瞬間就會切換。右 Alt 只在不按其他鍵、單獨按下再放開時才切換，因此 Alt 搭配其他按鍵的快速鍵仍可照常使用。",
    "zh-Hans": "韩/英键在按下的瞬间就会切换。右 Alt 只在不按其他键、单独按下再松开时才切换，因此 Alt 与其他按键组合的快捷键仍可照常使用。"
  },
  "한국어": {
    "en": "Korean (한국어)",
    "ja": "韓国語 (한국어)",
    "zh-Hant": "韓語 (한국어)",
    "zh-Hans": "韩语 (한국어)"
  },
  "한국어:": {
    "en": "한국어:",
    "ja": "한국어:",
    "zh-Hant": "한국어:",
    "zh-Hans": "한국어:"
  },
  "홈 폴더를 찾지 못해 설정을 저장할 수 없습니다.": {
    "en": "The home folder could not be found, so settings cannot be saved.",
    "ja": "ホームフォルダーが見つからないため、設定を保存できません。",
    "zh-Hant": "找不到家目錄（Home 資料夾），無法儲存設定。",
    "zh-Hans": "找不到主文件夹（Home），无法保存设置。"
  },
  "홈 폴더를 찾지 못해 저장하지 못했습니다.": {
    "en": "The home folder could not be found, so nothing was saved.",
    "ja": "ホームフォルダーが見つからないため、保存できませんでした。",
    "zh-Hant": "找不到家目錄（Home 資料夾），因此無法儲存。",
    "zh-Hans": "找不到主文件夹（Home），因此无法保存。"
  },
  "%1: 전환 순서 뒤로": {
    "en": "Move %1 later in the switching order",
    "ja": "%1 の切り替え順を後ろに移動",
    "zh-Hant": "將 %1 的切換順序往後移",
    "zh-Hans": "将 %1 的切换顺序往后移"
  },
  "%1: 전환 순서 앞으로": {
    "en": "Move %1 earlier in the switching order",
    "ja": "%1 の切り替え順を前に移動",
    "zh-Hant": "將 %1 的切換順序往前移",
    "zh-Hans": "将 %1 的切换顺序往前移"
  },
  "초": {
    "en": "seconds",
    "ja": "秒",
    "zh-Hant": "秒",
    "zh-Hans": "秒"
  },
  "표시 시간 (초)": {
    "en": "Display duration (seconds)",
    "ja": "表示時間（秒）",
    "zh-Hant": "顯示時間（秒）",
    "zh-Hans": "显示时间（秒）"
  }
};
// END TABLE

// One of `languages` for a tag such as "ja-JP", "zh_TW" or "zh-Hant-HK";
// "" when the tag is none of them.
function pick(tag) {
    var t = String(tag).replace(/_/g, "-").toLowerCase()
    if (/^ko(-|$)/.test(t)) {
        return "ko"
    }
    if (/^ja(-|$)/.test(t)) {
        return "ja"
    }
    if (/^en(-|$)/.test(t)) {
        return "en"
    }
    if (/^zh(-|$)/.test(t)) {
        // Traditional characters: the Hant script, or Taiwan, Hong Kong, Macau.
        if (t.indexOf("-hant") >= 0 || /-(tw|hk|mo)(-|$)/.test(t)) {
            return "zh-Hant"
        }
        return "zh-Hans"
    }
    return ""
}

// The display language: what was asked for, else the first of the system's
// languages that we have, else English.
function detect(requested, uiLanguages) {
    if (requested !== "" && pick(requested) !== "") {
        return pick(requested)
    }
    for (var i = 0; i < uiLanguages.length; ++i) {
        if (pick(uiLanguages[i]) !== "") {
            return pick(uiLanguages[i])
        }
    }
    return "en"
}

function requestedIn(args) {
    for (var i = 0; i < args.length; ++i) {
        if (String(args[i]).indexOf("--ui-language=") === 0) {
            return String(args[i]).substring("--ui-language=".length)
        }
    }
    return ""
}

var current = detect(requestedIn(Qt.application.arguments), Qt.locale().uiLanguages)
console.info("UI-LANGUAGE " + current)

// Source texts that lack a translation, as "language: text".
function incomplete() {
    var gaps = []
    for (var source in table) {
        for (var i = 1; i < languages.length; ++i) {
            if (!table[source][languages[i]]) {
                gaps.push(languages[i] + ": " + source)
            }
        }
    }
    return gaps
}

// The text in the display language. Korean is the source text itself.
function t(source) {
    if (current === "ko") {
        return source
    }
    var entry = table[source]
    if (entry === undefined || !entry[current]) {
        console.info("MISSING-TRANSLATION " + current + " " + source)
        return source
    }
    return entry[current]
}

// Switch the display language (for tests; the program never calls this).
function use(language) {
    current = language
}

// True when the display language is not the one the texts were written in.
function isDraft() {
    return current !== "ko"
}
