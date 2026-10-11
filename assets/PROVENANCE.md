# Provenance of `assets/`

Every file in `assets/` is a byte-for-byte copy of a file in the vendored original,
`upstream/SecondReality` (`mtuomi/SecondReality`, pinned at commit `071a82e`). Nothing here
is re-encoded, repaired or "cleaned": an asset the port cannot read as it ships is a finding
in `docs/fidelity/asset-inventory.md`, not something this tree fixes.

`assets/` mirrors the upstream layout exactly, so a copy's origin is its own path with
`assets/` replaced by `upstream/SecondReality/`. That is what makes the check in
`docs/fidelity/asset-inventory.md` §G a one-liner.

Licence: the original demo and its media are public domain under the Unlicense
(`upstream/SecondReality/UNLICENSE`); the port's own code is MIT (`LICENSE`).

## Files

Columns: `path` is the path under `assets/`; `origin` is the same file upstream; `commit` is
the upstream commit copied from; `parts` names the port's parts that need the file (see
`docs/fidelity/part-sequence.md` §1 for the sequence; `MAIN` is the shell and `START` the
setup menu); a later task's `recovered assets` table uses paths under the same tree.

| path | origin | commit | bytes | sha256 | licence | parts |
|---|---|---|---|---|---|---|
| `ALKU/FONA.UH` | `upstream/SecondReality/ALKU/FONA.UH` | `071a82e` | 45784 | `b17f1487c885ce0f9254a9b7f98316d17c27448d7348b734bf3539bbcbbb0522` | Unlicense / public domain | ALKU |
| `ALKU/FONA.LBM` | `upstream/SecondReality/ALKU/FONA.LBM` | `071a82e` | 15360 | `648b15245e7fe865d85494a976e696bdb00d1d6df9cb14dff01654220ac534e0` | Unlicense / public domain | ALKU |
| `ALKU/FONA2.LBM` | `upstream/SecondReality/ALKU/FONA2.LBM` | `071a82e` | 15268 | `ee03b242264f01e38ec8f0fbc59528def8ed91922c4603fb86fecdedb4b33825` | Unlicense / public domain | ALKU |
| `ALKU/HOI.U` | `upstream/SecondReality/ALKU/HOI.U` | `071a82e` | 128784 | `eb4e2c96f5f437c0e39cb581f8e464abae0bdc9761e977cdb142fd78ca65064b` | Unlicense / public domain | ALKU |
| `ALKU/HOI.LBM` | `upstream/SecondReality/ALKU/HOI.LBM` | `071a82e` | 33328 | `482ca11855d032fecda476ec3d6c099a1b14a793fa4afed4b4b293f0ffc8c5eb` | Unlicense / public domain | ALKU |
| `VISU/C/SCENE/U2A.00M` | `upstream/SecondReality/VISU/C/SCENE/U2A.00M` | `071a82e` | 796 | `02a05d721a63ebca33aa1613f0c60630c85300ac6258fa08874c9e43a9ae090e` | Unlicense / public domain | U2A |
| `VISU/C/SCENE/U2A.0AA` | `upstream/SecondReality/VISU/C/SCENE/U2A.0AA` | `071a82e` | 8 | `b15348c8f462384c01e83b6d499c6faf3f96808f5aa07c6bab4b65b36b4445d4` | Unlicense / public domain | U2A |
| `VISU/C/SCENE/U2A.0AB` | `upstream/SecondReality/VISU/C/SCENE/U2A.0AB` | `071a82e` | 16081 | `16cb21d60d680993f62db1e81e2db0e71b064e83aec1521c7ddd7a5cfc86c2f3` | Unlicense / public domain | U2A |
| `VISU/C/SCENE/U2A.001` | `upstream/SecondReality/VISU/C/SCENE/U2A.001` | `071a82e` | 8928 | `b2a0cfa9a7716c212c2fe1291632e5b7ddec26f2182611da5826cf94dd17732b` | Unlicense / public domain | U2A |
| `VISU/C/SCENE/U2A.002` | `upstream/SecondReality/VISU/C/SCENE/U2A.002` | `071a82e` | 7684 | `bb81e69bc0b2764e09e623f2d78ef451f63941db1630cadc55f7d77bc352633d` | Unlicense / public domain | U2A |
| `VISU/C/SCENE/U2A.003` | `upstream/SecondReality/VISU/C/SCENE/U2A.003` | `071a82e` | 2032 | `4bd72dc7e4ff2c8b00bae2c12a767944352d8c29ae5735a9336f425734899387` | Unlicense / public domain | U2A |
| `VISU/C/SCENE/U2E.00M` | `upstream/SecondReality/VISU/C/SCENE/U2E.00M` | `071a82e` | 900 | `9c44d67dbba895c7c039513a6dd56c919a55a796af3330fbfd7d04f6c95f5712` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.0AA` | `upstream/SecondReality/VISU/C/SCENE/U2E.0AA` | `071a82e` | 8 | `b15348c8f462384c01e83b6d499c6faf3f96808f5aa07c6bab4b65b36b4445d4` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.0AB` | `upstream/SecondReality/VISU/C/SCENE/U2E.0AB` | `071a82e` | 77069 | `b62428bee7c0b18322f1d8441fdb043716194bffe63f684bf3559559bd24a313` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.001` | `upstream/SecondReality/VISU/C/SCENE/U2E.001` | `071a82e` | 832 | `fc6e768e8522b5f757c54eda01b8328a319785ebfa82b6ec1216ff899581ec5b` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.002` | `upstream/SecondReality/VISU/C/SCENE/U2E.002` | `071a82e` | 1756 | `5fd170863972d667c8dc3f5b972e8912c80ac8269396cc7d36185ef5601fefec` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.003` | `upstream/SecondReality/VISU/C/SCENE/U2E.003` | `071a82e` | 836 | `b8de8cd3d1d5220f7a61245736f8b7f3b80ade5edbf801545bb5570fe3654765` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.004` | `upstream/SecondReality/VISU/C/SCENE/U2E.004` | `071a82e` | 800 | `404057e6a850f780e4d4ad1b543594e56e8e8f516d9f5456819c56fc5749d65e` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.005` | `upstream/SecondReality/VISU/C/SCENE/U2E.005` | `071a82e` | 916 | `64af3816196a30a9f8bd32ee5d6eafc8abe3fe84c7e7635212e6ddfe39474f4f` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.006` | `upstream/SecondReality/VISU/C/SCENE/U2E.006` | `071a82e` | 1292 | `712bc8532d4b06f46f60a7328d4882737b880a3b5e9b1451c4a4bd3a954d955e` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.007` | `upstream/SecondReality/VISU/C/SCENE/U2E.007` | `071a82e` | 1024 | `7c26e3ec943f78f21b34315261bd7b3f9a9b234434e2c2ee18ce5e765d2ea876` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.008` | `upstream/SecondReality/VISU/C/SCENE/U2E.008` | `071a82e` | 1260 | `91ff5ba4cdec4c0b09fd4cf445c75759076799d097fb10e91e62614de9233e56` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.009` | `upstream/SecondReality/VISU/C/SCENE/U2E.009` | `071a82e` | 4044 | `d4eeb3981db05d5123b6386272b31a04bc9b79d395a7cfe671a5df29395b4cd8` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.010` | `upstream/SecondReality/VISU/C/SCENE/U2E.010` | `071a82e` | 448 | `f7795b35eeba51c5361ebd95a59d3158056df7b9de5babe102d0c5c510ed76d7` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.011` | `upstream/SecondReality/VISU/C/SCENE/U2E.011` | `071a82e` | 532 | `a22b39be36efdd824e233ffbb7f22eabcf47decec4247087127aa7a7cb6ad024` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.012` | `upstream/SecondReality/VISU/C/SCENE/U2E.012` | `071a82e` | 1404 | `a570e3e75ca263ddec897e88f4af7d8ff27351c8ec04befc84d9588357ce5a60` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.013` | `upstream/SecondReality/VISU/C/SCENE/U2E.013` | `071a82e` | 796 | `61478c7e3fcb4e0008adf8c6a13eed50f88f1e4c7f37a4078e4ca66d8d0bfa33` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.014` | `upstream/SecondReality/VISU/C/SCENE/U2E.014` | `071a82e` | 564 | `5414b2bd8014a560453edadd6247bba017aea411a31698d970c58d4665623932` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.015` | `upstream/SecondReality/VISU/C/SCENE/U2E.015` | `071a82e` | 800 | `7239f53950d428649f251c6ceccb5985735eadf6c5ed652efb147ed82f8a0150` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.016` | `upstream/SecondReality/VISU/C/SCENE/U2E.016` | `071a82e` | 448 | `19df5d30564dd831a489bb5473ade7002182353e35432d5f835cecd8e635611d` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.017` | `upstream/SecondReality/VISU/C/SCENE/U2E.017` | `071a82e` | 1600 | `77895aefb234152c95da9b80bb3397df59880f5d8f3da4d40476b26297af5a1d` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.018` | `upstream/SecondReality/VISU/C/SCENE/U2E.018` | `071a82e` | 1432 | `5dd718d94404ae159804615116070a3af0ee692b58af096265f90d62f5f84661` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.019` | `upstream/SecondReality/VISU/C/SCENE/U2E.019` | `071a82e` | 796 | `eb0bc36db266bb631c390f88ed312cc2ad03842a8cddb099c3c60dee3c4ab121` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.020` | `upstream/SecondReality/VISU/C/SCENE/U2E.020` | `071a82e` | 756 | `b9ae0e37f8cf641aac4e1c38f6c3ea178b6f22d6afcf5ff0e945f8d762afa480` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.021` | `upstream/SecondReality/VISU/C/SCENE/U2E.021` | `071a82e` | 1760 | `9ee9c805beb583331b280d695fd5b7784241ed44f54ca7c19e82f4085f6b6819` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.022` | `upstream/SecondReality/VISU/C/SCENE/U2E.022` | `071a82e` | 448 | `a1075ea66b0defd533c6fbb1391a99b88bd625cefbbc9fc51f4c209c354a0630` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.023` | `upstream/SecondReality/VISU/C/SCENE/U2E.023` | `071a82e` | 12672 | `ce8228f1ba81cf8e61185f0f9395e46e27958ab5e70568acd586853620e1c958` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.024` | `upstream/SecondReality/VISU/C/SCENE/U2E.024` | `071a82e` | 3704 | `cb9e6abcdf71acef37eaba79f99b1dfc2f6e3e2e69250e38c0f6f5841f43fa63` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.025` | `upstream/SecondReality/VISU/C/SCENE/U2E.025` | `071a82e` | 796 | `8a1df3f1e75f36e34717f35e772705be6732102a223e4409114ce9ec6dc12678` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.026` | `upstream/SecondReality/VISU/C/SCENE/U2E.026` | `071a82e` | 1252 | `f8df6e06938a601974cc277042de8b799aad9897269d77a5e2bdd4480f2813c1` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.027` | `upstream/SecondReality/VISU/C/SCENE/U2E.027` | `071a82e` | 2212 | `f5e5afcf438703f3b5bd0404bef990eea9c555eff45fa317c8d5d8ef19e8a060` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.028` | `upstream/SecondReality/VISU/C/SCENE/U2E.028` | `071a82e` | 8968 | `9eb6c9d695e2f55e8f7f4fb48e3b5681caa3ad766c614b9c18c658963ff79c66` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.029` | `upstream/SecondReality/VISU/C/SCENE/U2E.029` | `071a82e` | 796 | `ea7651d21f1f7d57feca832eb1836ac33050e04972cdaccad995758250a966ef` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.030` | `upstream/SecondReality/VISU/C/SCENE/U2E.030` | `071a82e` | 1756 | `6955b29b590b29fe60eb761d299002b3dccaeba4ee359fe44b0d28cd82f98578` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.031` | `upstream/SecondReality/VISU/C/SCENE/U2E.031` | `071a82e` | 448 | `eda681b2638113024335b1c3e8a3364a6fb12d84645a674cbb5fa422eacf2e0f` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.032` | `upstream/SecondReality/VISU/C/SCENE/U2E.032` | `071a82e` | 1572 | `8e3eba5bf4427b29f704fe3ac2693352c74224ed1285f5462d538f262d3f88e9` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.033` | `upstream/SecondReality/VISU/C/SCENE/U2E.033` | `071a82e` | 796 | `112a80ec37a23057d130fffa9d68fa3c4999736c9784a62fec0c97d0c656684b` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.034` | `upstream/SecondReality/VISU/C/SCENE/U2E.034` | `071a82e` | 4092 | `4e2e0d0ff008177bd35220e24f60459dd78bfb38286dd8f701b88a1253273983` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.035` | `upstream/SecondReality/VISU/C/SCENE/U2E.035` | `071a82e` | 1048 | `0bb3a6592e59a8ebfe7c1ceb63963d2fa28ff346b9572b25807288a7a9d68229` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.036` | `upstream/SecondReality/VISU/C/SCENE/U2E.036` | `071a82e` | 3696 | `d5177365ac263321987302c86465e0a97aa0368cf74443dd31d2b9931819fd09` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.037` | `upstream/SecondReality/VISU/C/SCENE/U2E.037` | `071a82e` | 2716 | `d810d69d2b4916a39cce76194be3510e63c089a1306de36d8ff099e376ea04bd` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.038` | `upstream/SecondReality/VISU/C/SCENE/U2E.038` | `071a82e` | 3356 | `a030ab53e64274f46841746d1e88efdfcabb4b63a15de92fc4dae17b8ba3e7c6` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.039` | `upstream/SecondReality/VISU/C/SCENE/U2E.039` | `071a82e` | 1900 | `e1e284133d30d0e84c9d34cbfee98a146b967a1325c304c969bd8b3beeb7b5a7` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.040` | `upstream/SecondReality/VISU/C/SCENE/U2E.040` | `071a82e` | 1116 | `65f62bba7bf694de2666c78f568668f5124949ab4df8e2a8c1b250d4909faa08` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.041` | `upstream/SecondReality/VISU/C/SCENE/U2E.041` | `071a82e` | 796 | `57775d45ae878d9a27ed14412600935c11c946cbe1265ec22c598702eb0b00e4` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.042` | `upstream/SecondReality/VISU/C/SCENE/U2E.042` | `071a82e` | 796 | `a13e726dedc4ea067b6535a4c794a9b27414c5879a58b11d8bc528aa1aa85f0c` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2A.LBM` | `upstream/SecondReality/VISU/C/SCENE/U2A.LBM` | `071a82e` | 5916 | `957f9ad4a9ffe4e49b42a3a42151e362b77af3636b3dccf0e9ee0ffb37d4b9b3` | Unlicense / public domain | U2A |
| `VISU/C/SCENE/U2A.MAT` | `upstream/SecondReality/VISU/C/SCENE/U2A.MAT` | `071a82e` | 304 | `65ebed2b6954c7e5407fbf4afc809163b005182a84e30f864d031cb5aba4a671` | Unlicense / public domain | U2A |
| `VISU/C/SCENE/U2A.INF` | `upstream/SecondReality/VISU/C/SCENE/U2A.INF` | `071a82e` | 19 | `648c45db11c875833b86f9c300eee10fb7eb98757f0bfc22f289888c1f52d1fc` | Unlicense / public domain | U2A |
| `VISU/C/SCENE/U2E.LBM` | `upstream/SecondReality/VISU/C/SCENE/U2E.LBM` | `071a82e` | 5916 | `a0a629bb5cd40c766db0ee7b275447d69d0a3cceeae52c44f0c9bfa1ab17ee15` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.MAT` | `upstream/SecondReality/VISU/C/SCENE/U2E.MAT` | `071a82e` | 480 | `17e938439b3ca076c8be9084abfce0fb80757329ca5ec6aab0ffb91975575811` | Unlicense / public domain | U2E |
| `VISU/C/SCENE/U2E.INF` | `upstream/SecondReality/VISU/C/SCENE/U2E.INF` | `071a82e` | 19 | `58aa996d2def9ea380ecc9033d3e2adf37e70e984900eb663030ab90c971c235` | Unlicense / public domain | U2E |
| `VISU/C/U2ABG.UH` | `upstream/SecondReality/VISU/C/U2ABG.UH` | `071a82e` | 64784 | `3016e5b28ae4b06842a87fa1637d57e7277221d035d84a4f16cad6459b786da2` | Unlicense / public domain | U2A |
| `VISU/C/U2ABG.LBM` | `upstream/SecondReality/VISU/C/U2ABG.LBM` | `071a82e` | 19906 | `6d399af7b3bf0f36776fec29ceab924596f8c5aa8486650fb93dade133f2c383` | Unlicense / public domain | U2A |
| `VISU/C/U2A.PAL` | `upstream/SecondReality/VISU/C/U2A.PAL` | `071a82e` | 768 | `09ac30cadf94de7476ec450d9993e1ff530883e63501acc9d8cab625223d9e28` | Unlicense / public domain | U2A |
| `VISU/C/SCENE/U2E.PAL` | `upstream/SecondReality/VISU/C/SCENE/U2E.PAL` | `071a82e` | 768 | `7cff56b7049bcbe52509c21aa586c6e3738bb0b9c9aeaa4644f5351a3bfc889f` | Unlicense / public domain | U2E |
| `PAM/OUT.IN0` | `upstream/SecondReality/PAM/OUT.IN0` | `071a82e` | 198707 | `b601f03ea982e2a50171ad364ae50f45029cc86063bfafef04894113733e5bf2` | Unlicense / public domain | PAM |
| `PAM/OUT.IN1` | `upstream/SecondReality/PAM/OUT.IN1` | `071a82e` | 192299 | `d49ce464c8f89206b894cf5879b09d0d6e5f95ce8769c9cb54e0fd9730e98a30` | Unlicense / public domain | PAM |
| `PAM/OUT.IN2` | `upstream/SecondReality/PAM/OUT.IN2` | `071a82e` | 190153 | `f9979b3831727020dd96a238eebea5f82d3d278edf1b9dcbe4db2236c15edc4c` | Unlicense / public domain | PAM |
| `PAM/OUT.IN3` | `upstream/SecondReality/PAM/OUT.IN3` | `071a82e` | 189960 | `28d9185e3b66d4916c65df07365092ab38e6f4848c8df34dae1cfbe6568d3212` | Unlicense / public domain | PAM |
| `PAM/OUT.IN4` | `upstream/SecondReality/PAM/OUT.IN4` | `071a82e` | 179997 | `05433f19979a1c28f521e06a24a0e4ebf07a83fc4bfac4736576abf96216540e` | Unlicense / public domain | PAM |
| `PAM/OUT.ANI` | `upstream/SecondReality/PAM/OUT.ANI` | `071a82e` | 324625 | `321d1142c5410bb0523787470a52651e282f92e76c89795c0b33d59c57c9c458` | Unlicense / public domain | PAM |
| `PAM/PAL.PAL` | `upstream/SecondReality/PAM/PAL.PAL` | `071a82e` | 768 | `cb8c0d48b62cd9fb9a96b63a30967d2243e8b035deec39efaffcdd68dc75de00` | Unlicense / public domain | PAM |
| `PAM/PRAX4.FLI` | `upstream/SecondReality/PAM/PRAX4.FLI` | `071a82e` | 335476 | `7316aece20274d7e7fda80eed4707ca1e3d320a49d508e462216c68ff4f5a4e0` | Unlicense / public domain | PAM |
| `BEG/SRTITLE.UP` | `upstream/SecondReality/BEG/SRTITLE.UP` | `071a82e` | 44575 | `ad251af7b501dd6e24637bd0768dd24070380361369ede32d74f071b67ece28d` | Unlicense / public domain | BEGLOGO |
| `BEG/SRTITLE.LBM` | `upstream/SecondReality/BEG/SRTITLE.LBM` | `071a82e` | 53254 | `ba40adf1f0d546f22ef4276da301dc8d4a4884f5f4bd1d75036e6fcd26e2f5c4` | Unlicense / public domain | BEGLOGO |
| `GLENZ/FC.UH` | `upstream/SecondReality/GLENZ/FC.UH` | `071a82e` | 64784 | `c63d6290b66b679440643e003cc879fa8e85c9a78ef723e1a2f913c39e3ac15e` | Unlicense / public domain | GLENZ |
| `GLENZ/FC.LBM` | `upstream/SecondReality/GLENZ/FC.LBM` | `071a82e` | 25552 | `8ccc36046013c9bb36407680e6c4a8f38a7e961212784b610353d6674f6bb2da` | Unlicense / public domain | GLENZ |
| `TUNNELI/TUNNEL.DAT` | `upstream/SecondReality/TUNNELI/TUNNEL.DAT` | `071a82e` | 35328 | `7ae9ab3ca755a60709451e4e1d8e11944ad49df022e3bc30f3772acb0e698e33` | Unlicense / public domain | TUNNELI |
| `TUNNELI/SINIT.DAT` | `upstream/SecondReality/TUNNELI/SINIT.DAT` | `071a82e` | 12292 | `282ac11e5943b4d0a775f95e67830a244f5872ae92b859278ddd2ed9cd4e31a3` | Unlicense / public domain | TUNNELI |
| `TECHNO/TROLL.UP` | `upstream/SecondReality/TECHNO/TROLL.UP` | `071a82e` | 70718 | `6a9fe2c5ec513b9dd33912ad7fd7ec6606be0ffce0a0c97332731074bc07032e` | Unlicense / public domain | TECHNO |
| `TECHNO/PANICPIC.LBM` | `upstream/SecondReality/TECHNO/PANICPIC.LBM` | `071a82e` | 82456 | `9d9e4d73bc9afe7584aa276d01b1cc877aa502aa05e92cde7923120aa443babe` | Unlicense / public domain | TECHNO |
| `PANIC/MONSTER.U` | `upstream/SecondReality/PANIC/MONSTER.U` | `071a82e` | 64000 | `47ea1ec93a1e78fc0b36aae1ea6180cdcbd2564cb37f685ff1898bcf0c3304d6` | Unlicense / public domain | PANICEND |
| `PANIC/MONSTER.PAL` | `upstream/SecondReality/PANIC/MONSTER.PAL` | `071a82e` | 768 | `78ee356861fc8f97c3aff43cbd96286e5c3dcc12d18dd024fb9a1b8c0edb3fac` | Unlicense / public domain | PANICEND |
| `FOREST/O2.SCI` | `upstream/SecondReality/FOREST/O2.SCI` | `071a82e` | 21258 | `81693b533a77bca4ededc840042486693920b108e1d032d46cab6f81df40386e` | Unlicense / public domain | MNTSCRL |
| `FOREST/HILLBACK.CLX` | `upstream/SecondReality/FOREST/HILLBACK.CLX` | `071a82e` | 64778 | `505186e04211e0588beb187fa03f8a34a1e339c5dba5e186e49aa16415cf3ac4` | Unlicense / public domain | MNTSCRL |
| `FOREST/POS1.DAT` | `upstream/SecondReality/FOREST/POS1.DAT` | `071a82e` | 25088 | `1ccaefce6474102a4cb2c9b094946a5daf8cd88061af3bb8f53ef2775ea4b5d7` | Unlicense / public domain | MNTSCRL |
| `FOREST/POS2.DAT` | `upstream/SecondReality/FOREST/POS2.DAT` | `071a82e` | 25064 | `73f6a5db6cd580352ba96ea29045149be49500e986e831a369ddda48b8be4980` | Unlicense / public domain | MNTSCRL |
| `FOREST/POS3.DAT` | `upstream/SecondReality/FOREST/POS3.DAT` | `071a82e` | 25114 | `6ae248adaf228f35cb39644f699d6d0f25bcbe80d14a98fe41cbe945755f3b84` | Unlicense / public domain | MNTSCRL |
| `LENS/LENS.EX0` | `upstream/SecondReality/LENS/LENS.EX0` | `071a82e` | 215 | `ab4e2ac886fd12bce80d6d5aa897d98b946b9f22485ba823d9ecba4595bcd4c3` | Unlicense / public domain | LNS&ZOOM |
| `LENS/LENS.EX1` | `upstream/SecondReality/LENS/LENS.EX1` | `071a82e` | 19588 | `dec0520aa182b7fffc2af9d346af363d63c1a83f740828522a8595dbe7989bbb` | Unlicense / public domain | LNS&ZOOM |
| `LENS/LENS.EX2` | `upstream/SecondReality/LENS/LENS.EX2` | `071a82e` | 6860 | `0fedf56cb2884a8856b1dbd711ba7051e7b8a608124776144bd48343837d9996` | Unlicense / public domain | LNS&ZOOM |
| `LENS/LENS.EX3` | `upstream/SecondReality/LENS/LENS.EX3` | `071a82e` | 1538 | `fe06ed50dfc9a44f157eb9db116561596336155f01c18f1ba8d4968984ebf9db` | Unlicense / public domain | LNS&ZOOM |
| `LENS/LENS.EX4` | `upstream/SecondReality/LENS/LENS.EX4` | `071a82e` | 6252 | `f7a480dc71739f44adf559ef348f976ceb7ab623241d5cd4432902220d7c29f5` | Unlicense / public domain | LNS&ZOOM |
| `LENS/LENS.EXB` | `upstream/SecondReality/LENS/LENS.EXB` | `071a82e` | 64784 | `4f76b23b3931ee15b4b348978bb5b51794a0137de76b6f40ea7d8644f1b267dc` | Unlicense / public domain | LNS&ZOOM |
| `LENS/LENS.EXP` | `upstream/SecondReality/LENS/LENS.EXP` | `071a82e` | 18864 | `052882cb6f6663782e7cb8c09fadc0f0fdc1cfdb5e8c9ac73762f43dacb7bdc2` | Unlicense / public domain | LNS&ZOOM |
| `LENS/FILLER` | `upstream/SecondReality/LENS/FILLER` | `071a82e` | 4224 | `1209fc5b6d1e52fe4f453d6f5a2251ce6274856764c59f564574f70885d5d287` | Unlicense / public domain | LNS&ZOOM |
| `LENS/LENSPIC.LBM` | `upstream/SecondReality/LENS/LENSPIC.LBM` | `071a82e` | 26708 | `968290305583c5fd4a25e1101b5a453eb5c515886689b4b612448f2b0ab9348a` | Unlicense / public domain | LNS&ZOOM |
| `LENS/LENS.U` | `upstream/SecondReality/LENS/LENS.U` | `071a82e` | 64000 | `bc16dc40f6e3e5d07cd3831b163aa61048da9a53ae74ca96aa90b7a2bbcfe7b6` | Unlicense / public domain | LNS&ZOOM |
| `WATER/BKG.CLX` | `upstream/SecondReality/WATER/BKG.CLX` | `071a82e` | 64778 | `d911d7f64d5dc5bc412e172177bcfe77fa2d2a03961a1cded774387cdc9523e3` | Unlicense / public domain | RAYSCRL |
| `WATER/MIEKKA.SCI` | `upstream/SecondReality/WATER/MIEKKA.SCI` | `071a82e` | 14778 | `3c98ad931328605c9009c4b078b7b29e74575f9a8bbcea0446d4009dcf4a48a2` | Unlicense / public domain | RAYSCRL |
| `WATER/WAT1.DAT` | `upstream/SecondReality/WATER/WAT1.DAT` | `071a82e` | 23096 | `4a454f4e529fe1617c1b6cafcd0b7852f198eb870e37f52da510f2dc5201e800` | Unlicense / public domain | RAYSCRL |
| `WATER/WAT2.DAT` | `upstream/SecondReality/WATER/WAT2.DAT` | `071a82e` | 23034 | `dc513b4f06d2876a28963f4d1e8b1c509d4fff1c65b28460f031d1ce6210f3e7` | Unlicense / public domain | RAYSCRL |
| `WATER/WAT3.DAT` | `upstream/SecondReality/WATER/WAT3.DAT` | `071a82e` | 27818 | `b7af775c7247e2eed95800a68f8901ab641e735d896bbf7a79c85731cd38e27b` | Unlicense / public domain | RAYSCRL |
| `COMAN/COMBG.UH` | `upstream/SecondReality/COMAN/COMBG.UH` | `071a82e` | 32768 | `856422db6d71bdce5fc362813c91a0b3b998995df6685e3a380ae259fe2f9f57` | Unlicense / public domain | 3DSINFLD |
| `COMAN/COMBG.X` | `upstream/SecondReality/COMAN/COMBG.X` | `071a82e` | 64016 | `89283f978c4349a0311a862494f11490c1b6916468e7f096f4682abf65ccf65f` | Unlicense / public domain | 3DSINFLD |
| `COMAN/COMBG.LBM` | `upstream/SecondReality/COMAN/COMBG.LBM` | `071a82e` | 1796 | `5796e74aa3d7d9bb476cbedabfb8534ab0720e41f93de3f7491afe06938d6613` | Unlicense / public domain | 3DSINFLD |
| `COMAN/W1DTA.BIN` | `upstream/SecondReality/COMAN/W1DTA.BIN` | `071a82e` | 65536 | `3cca133f1eaa7b1dd714688959d477fad401919270cb2cbaaf3faea42db42410` | Unlicense / public domain | 3DSINFLD |
| `COMAN/W2DTA.BIN` | `upstream/SecondReality/COMAN/W2DTA.BIN` | `071a82e` | 65536 | `dcff14a8420a66603b5e17fa93a232187c74dc944cb7cf96c58c90e0ae387d61` | Unlicense / public domain | 3DSINFLD |
| `JPLOGO/ICEKNGDM.UP` | `upstream/SecondReality/JPLOGO/ICEKNGDM.UP` | `071a82e` | 58566 | `d24f095d95659a2ee38460644c8efa6bc172897d8206e20e1b50382757570ab6` | Unlicense / public domain | JPLOGO |
| `JPLOGO/ICEKNGDM.LBM` | `upstream/SecondReality/JPLOGO/ICEKNGDM.LBM` | `071a82e` | 66862 | `2ac0926777e2e6198265eef1b35ff06110aab458d88e2bb4881bbedc14defaa9` | Unlicense / public domain | JPLOGO |
| `DDSTARS/TEXTS.16` | `upstream/SecondReality/DDSTARS/TEXTS.16` | `071a82e` | 16064 | `4a582ca51442dc2afb7f822c9b655c13361c86b4fc5dd22bcbda2805eab37912` | Unlicense / public domain | DDSTARS |
| `DDSTARS/TEXTS.LBM` | `upstream/SecondReality/DDSTARS/TEXTS.LBM` | `071a82e` | 21784 | `330f14c3f46f6274aef64723d6b25ba10684c1785a9392eb6ed46318263731eb` | Unlicense / public domain | DDSTARS |
| `END/PIC.UH` | `upstream/SecondReality/END/PIC.UH` | `071a82e` | 45884 | `551b7e733ad9ae56e9fae3157fddad470b5561320c970e79d5eac5d6c4a1066f` | Unlicense / public domain | ENDLOGO |
| `END/PIC.LBM` | `upstream/SecondReality/END/PIC.LBM` | `071a82e` | 53576 | `fb5172fccff7f02f4b7396be10a47cde02d1d8a6daec16be1299285bc9fa0e67` | Unlicense / public domain | ENDLOGO |
| `CREDITS/PICS/PIC01.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC01.LBM` | `071a82e` | 7878 | `3a611f9fbf0ac61f5cdf05bf093f7973a9081ba03a873f2c77b68cb9305680a1` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC02.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC02.LBM` | `071a82e` | 7000 | `51391adee82deb783e636fe503080cd07fd817b7f376b7e583868fc032ead3b0` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC03.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC03.LBM` | `071a82e` | 12078 | `162b19b3a5a3c4346c2443850181570bcf82c701c7118fd68229a48e01a483e4` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC04.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC04.LBM` | `071a82e` | 12202 | `0b23837f33aa79b25875106bd10d6cfd06d5463138ac769260369fbc7839d4d9` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC05.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC05.LBM` | `071a82e` | 9962 | `888f41812c79561d53630b667bb1e57cdd1da20c3a0e50bb7af4f973d100a1f0` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC05B.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC05B.LBM` | `071a82e` | 11524 | `0099fefa4ae92d8f32189bf6a35044c48a75e7e40208b926d9a8690c4e5005e0` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC06.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC06.LBM` | `071a82e` | 15092 | `ccdff172f9ef413fe6ded56100b2e2d560b04150e4aec2a9f628b69e22d6027c` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC07.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC07.LBM` | `071a82e` | 17400 | `0a6c1f1bcfd7a974d404b54b90f20a88af1d011f11da3275c7d2736c7431bb7a` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC08.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC08.LBM` | `071a82e` | 17156 | `fbd803b64bdf337627a1cad978d167361812b49444e567b50bc6ab65da233947` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC09.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC09.LBM` | `071a82e` | 16474 | `44a1f15db718d3cffd0dc07fba2b102dfae4ad8dab4ec70389fad750858c621c` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC10.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC10.LBM` | `071a82e` | 12986 | `289ff3c0b3306a6057957bf6e43fd5294e206f77b4eb8aa7cc5998ce4d540b7f` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC10B.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC10B.LBM` | `071a82e` | 16658 | `91171c8f444b072cfddccb5e40710671f45ad59b9db92b0dcd06e3a599c40473` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC11.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC11.LBM` | `071a82e` | 11754 | `2bfc417607b498135e17a3cef287b6bc483651610606cc5f5855463da3cbad5f` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC12.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC12.LBM` | `071a82e` | 13982 | `92a2b92383b7e0e9f78e69edb78342976cef2ebde19f54e18637642eb2e48448` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC13.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC13.LBM` | `071a82e` | 9762 | `6ce71dbbd19b32f94979c7544aa6838fb6f18a165f39e2f3743b052e4c9794c6` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC14.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC14.LBM` | `071a82e` | 15478 | `8948918eb924fa183feedeb501264149122d86f7cf36e99191a455de3cd2a100` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC14B.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC14B.LBM` | `071a82e` | 8298 | `7482125e0bb271613112b39c3914075d94e866f0fc0d0a19c9cee150c8d2ddd8` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC15.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC15.LBM` | `071a82e` | 13408 | `672556bbc53bd0a35b347f79cf425e64d85b83124839a435c2995ef3873e34b2` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC16.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC16.LBM` | `071a82e` | 5512 | `9c01f3aaab0efbb181ac5e1e222ed597c2e3f563755f46063684e5a86faca0b7` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC17.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC17.LBM` | `071a82e` | 9182 | `ce83a0a41af0513f42205eb7e766f1bf44bf13290c1a4c57d54dada4ea608ef3` | Unlicense / public domain | CRED |
| `CREDITS/PICS/PIC18.LBM` | `upstream/SecondReality/CREDITS/PICS/PIC18.LBM` | `071a82e` | 4684 | `5e2d0867873de88aa889e15127af47cbc21abde98a3f1ecfd971426efd8df813` | Unlicense / public domain | CRED |
| `ENDSCRL/FONA.UH` | `upstream/SecondReality/ENDSCRL/FONA.UH` | `071a82e` | 45784 | `cb2529a277f22e77509c96959b8e3ac322150444d8c33fe7c58d8fec0eac4338` | Unlicense / public domain | ENDSCRL |
| `ENDSCRL/FONA.LBM` | `upstream/SecondReality/ENDSCRL/FONA.LBM` | `071a82e` | 9730 | `d74d27312416c40a101d6bb7022c59172c875e9a87ef284b2de0045a9e9d0f7c` | Unlicense / public domain | ENDSCRL |
| `ENDSCRL/ENDSCROL.TXT` | `upstream/SecondReality/ENDSCRL/ENDSCROL.TXT` | `071a82e` | 1007 | `a595930f0b394d03a15799274ceb86d4ec427368d575cdbe1de1adafdf014805` | Unlicense / public domain | ENDSCRL |
| `MAIN/DATA/ENDSCROL.TXT` | `upstream/SecondReality/MAIN/DATA/ENDSCROL.TXT` | `071a82e` | 6141 | `11067c5f245a22d6a50ad366f312e562caad6958d1f73147eb667c8f2de37bed` | Unlicense / public domain | ENDSCRL |
| `MAIN/MUSIC0.S3M` | `upstream/SecondReality/MAIN/MUSIC0.S3M` | `071a82e` | 391312 | `aacdb968ae9ebf07bfa02af129fb43f1b15101ab3a94c95b7f5fe69840d28fe1` | Unlicense / public domain | MAIN |
| `MAIN/MUSIC1.S3M` | `upstream/SecondReality/MAIN/MUSIC1.S3M` | `071a82e` | 600860 | `3393f848a4840b53aa247c4158e414bad848879e7f6059e214631c310e8dde54` | Unlicense / public domain | MAIN |
| `MAIN/DATA/U2END.BIN` | `upstream/SecondReality/MAIN/DATA/U2END.BIN` | `071a82e` | 4000 | `4c2ae2970efe756d3fcceeb9a1012c4e04fd9eb85308e890fb8690042be99cf4` | Unlicense / public domain | MAIN |
| `START/HZPIC.P` | `upstream/SecondReality/START/HZPIC.P` | `071a82e` | 60706 | `c284e2af320e61b04de73e7e0f943217caae0c3d1bca419bd9bfb9bd7d9f347b` | Unlicense / public domain | START |
| `START/HZPIC.PAL` | `upstream/SecondReality/START/HZPIC.PAL` | `071a82e` | 768 | `9654e10d4d024f944b8146b3ebe907140dc418f251c833882d5202d200c200d9` | Unlicense / public domain | START |
| `START/HZPIC.LBM` | `upstream/SecondReality/START/HZPIC.LBM` | `071a82e` | 59352 | `ec3d65b55103fea4570561d7bac04291f2888098c226047db2122e7dc0e38ffb` | Unlicense / public domain | START |
| `GRAB/BLOWFNT.1BM` | `upstream/SecondReality/GRAB/BLOWFNT.1BM` | `071a82e` | 64000 | `a822ac2c9ebaaa3afbc1fc82750313b59718c8a9c729827e8f735b8f5c4bea50` | Unlicense / public domain | GRAB |
| `GRID/EYE4.UH` | `upstream/SecondReality/GRID/EYE4.UH` | `071a82e` | 64784 | `a49726062aa91c4bb4cc45ca641727797d2acb07e98a4d310f370fc576b65f4c` | Unlicense / public domain | GRID |

### Notes on individual files

Facts about a file that a reader of the table above would otherwise have to rediscover. They
are keyed by `path`; the table's columns are fixed and are not extended for them.

- `COMAN/COMBG.UH` — **truncated, and copied exactly as it ships.** It is byte-for-byte
  `COMAN/COMBG.X[:32768]`; its header promises 64,000 payload bytes and it carries 32,752, so
  it does not decode as a whole image and a loader must refuse it rather than clip it. It is
  nonetheless the file `COMAN/_COMBG.OBK` embeds, and it holds every byte
  `COMAN/MAIN.C:163,170,174` reads (the loops at `:166-176` stop at row 89).
  `docs/fidelity/asset-inventory.md` §E.2 has the numbers.
- `PAM/OUT.IN0`–`OUT.IN4` — ASCII `INCBIN` fragments, not binaries: they are the text form of
  `PAM/OUT.ANI` (verified: parsing the `db` values reproduces that file's bytes). The binary
  is in the tree beside them and is the file the port loads.
- `MAIN/MUSIC0.S3M`, `MAIN/MUSIC1.S3M` — the S3M magic is `SCRM` at offset 44, not at offset
  0; their first bytes are the module title (`UnreaL ][ …`). Their packed pattern data is
  XOR-scrambled (decision D5).
- `ENDSCRL/ENDSCROL.TXT` (1,007 bytes) and `MAIN/DATA/ENDSCROL.TXT` (6,141 bytes) are
  **different files**: the first is the build directory's test text, the second is the text
  the shipped pack carries (`MAIN/PACKFINA.INC:50`) and the one the port must scroll. Both are
  copied because both are cited; §E.9.
- `ENDPIC/` — the alternate build of part 4 is not copied as a set. The files it shares with
  other directories are in the table above under the path that a shipped build names; §E.10.

## Collapsed duplicates

One file, one path. Each row below is a path upstream that is byte-identical to a file
already in `assets/`, so it was **not** copied; the `collapsed onto` column names the copy
that carries the same bytes. Verified with `sha256sum`; the digests here are also in
`docs/fidelity/asset-inventory.md` §B.

| dropped path | collapsed onto | sha256 | why it is safe to drop |
|---|---|---|---|
| `MAIN/DATA/O2.SCI` | `assets/FOREST/O2.SCI` | `81693b533a77bca4ededc840042486693920b108e1d032d46cab6f81df40386e` | the pack's staging copy; `FOREST/O2.SCI` is the file `FOREST/READ2.PAS:110` opens |
| `MAIN/DATA/TROLL.UP` | `assets/TECHNO/TROLL.UP` | `6a9fe2c5ec513b9dd33912ad7fd7ec6606be0ffce0a0c97332731074bc07032e` | the pack's staging copy; `TECHNO/KOE.C:605` opens `troll.up` |
| `MAIN/DATA/U2A.00M`, `.0AA`, `.0AB`, `.001`–`.003` | `assets/VISU/C/SCENE/U2A.*` | see §B.4 | staged copies of the U2A scene set; identical to `VISU/C/SCENE/U2A.*` |
| `MAIN/DATA/U2E.00M`, `.0AA`, `.0AB`, `.001`–`.042` | `assets/VISU/C/SCENE/U2E.*` | see §B.4 | staged copies of the U2E scene set; identical to `VISU/C/SCENE/U2E.*` |
| `END/NUTS.LBM` | `assets/END/PIC.LBM` | `fb5172fccff7f02f4b7396be10a47cde02d1d8a6daec16be1299285bc9fa0e67` | the two names are one file; `END/DOPIC.BAT:1` names `pic.lbm` |
| `VISU/C/CITY.PAL` | `assets/VISU/C/U2A.PAL` | `09ac30cadf94de7476ec450d9993e1ff530883e63501acc9d8cab625223d9e28` | the city tool's copy of the scene palette |
| `VISU/C/SCENE/U2A.PAL` | `assets/VISU/C/U2A.PAL` | `09ac30cadf94de7476ec450d9993e1ff530883e63501acc9d8cab625223d9e28` | a second copy of the same palette inside the scene directory |
| `GLENZ/EYE4.UH` | `assets/GRID/EYE4.UH` | `a49726062aa91c4bb4cc45ca641727797d2acb07e98a4d310f370fc576b65f4c` | `GLENZ/_BGPIC.OBK`'s payload is this file, but GLENZ's link line (`GLENZ/MAKEFILE:3`) does not name it; `GRID/_EYE.OBK` does, and GRID is the part that shows the eye |

### Identical files that are not copied at all

None of these is linked, loaded or converted by any shipped part, so nothing needs them;
their duplicate status is recorded in `docs/fidelity/asset-inventory.md` §B rather than
resolved here.

| path(s) | sha256 | note |
|---|---|---|
| `BEG/TMP.BBM`, `END/TMP.BBM`, `ENDPIC/TMP.BBM`, `JPLOGO/TMP.BBM` | `1fa068ca80039120a22de4605fbb5b01f4b7a058b72dc8ba0254f26dc77aef9c` | four identical copies of one picture; no part names it |
| `ENDPIC/PIC.UH`, `JPLOGO/PIC.UH` | `89e821e6dd7d2edaa70c9ba4a7a5417da0e133532bf68ac10784e110f6ba7844` | identical to each other; neither is the file its directory's build links (`ENDPIC/_PIC.OBK` reconstructs `SRTITLE.U`, `JPLOGO/_PIC.OBK` reconstructs `ICEKNGDM.UP`) |
| `ENDPIC/PIC.LBM`, `JPLOGO/PIC.LBM` | `a4f0705c91bc7b4f6818b837f23cd1874c723820f8612f4949949871f056c9da` | source art of the pair above |
| `COMAN/SIN1024.INC`, `DDSTARS/SIN1024.INC`, `DOTS/SIN1024.INC`, `GLENZ/SIN1024.INC`, `HARD/SIN1024.INC`, `LENS/SIN1024.INC`, `TECHNO/SIN1024.INC`, `TWIST/SIN1024.INC` | `fa6d0a6a87499e5fca9eb5b74a6254be0fc43ba78c847d79a97225137ff8f959` | eight identical copies of one sine table (a source fragment, not an asset the port loads) |
| `JPLOGO/SIN1024.INC` | `76ee2420dee3e005a8a990561c7307e5f8fa32f56312fba7ec9cac34213b1261` | the ninth copy, and the one that differs — see §B.7 |

`ENDPIC/` is the alternate build of part 4 (§1 of the inventory explains why it is not
copied as a set).

## Recovered assets

A later task (A5) recovers assets that exist only inside the shipped binaries, and appends
its own table here:

```
| path | blob | record | range | sha256 |
```

Its rows are files written into `assets/` by that task; the table above is not edited.
