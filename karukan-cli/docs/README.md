# Notice

システム辞書 (`dict.bin`) は、次の 3 つの辞書データを `karukan-dict build` で重ねて構築しています
（先のものほど優先。`scripts/build-dict.sh` が取得と構築を行います）。

1. **Mozc の辞書** (`src/data/dictionary_oss/dictionary00〜09.txt`)
   <https://github.com/google/mozc>
   並び順（コスト）と日常語彙の土台です。IPAdic ライセンスと BSD 3-Clause License の下で
   配布されています。原文を [LEGAL-mozc](./LEGAL-mozc) に写しています。
2. **SudachiDict** (small / core / notcore)、株式会社ワークスアプリケーションズ
   <https://github.com/WorksApplications/SudachiDict>
   固有名詞と長い複合語の補完です。Apache License, Version 2.0 の下で配布されています。
   [LEGAL](./LEGAL) は SudachiDict リポジトリからコピーしたものです。元の CSV に対して
   顔文字（ＡＡ）の除外などの前処理をしています。
3. **dic-nico-intersection-pixiv**、ncaq
   <https://github.com/ncaq/dic-nico-intersection-pixiv>
   ニコニコ大百科とピクシブ百科事典の双方にある見出し語の辞書です。作者はコードを MIT
   ライセンスとし、生成物については「スクレイピング結果を利用している都合上、著作権は
   主張しない」としています。元データはニコニコ大百科 <https://dic.nicovideo.jp/> と
   ピクシブ百科事典 <https://dic.pixiv.net/> です。

本辞書バイナリは、上記のうち最も制約の強い Apache License, Version 2.0 の下で配布しています。
詳細は [LICENSE-2.0.txt](./LICENSE-2.0.txt) をご覧ください。
