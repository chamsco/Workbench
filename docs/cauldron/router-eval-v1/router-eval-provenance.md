# Router eval: provenance

All texts are Charles's own messages in the project chat and its threads, copied exactly (no trimming, no spelling or language fixes). There are no Cauldron runtime traces. The earlier "30-task inventory" was not used.

Each row in the `.jsonl` files carries `source_message_id`, `created_at`, `where`, `chars`, `text_sha256` and `source_fetch`. Every text was fetched by message id with the project's message-read tool and compared byte for byte with the draft's text; 4 draft texts (the ~8 KB-truncated ones) differ and were replaced by the full fetch. 24 texts the draft had taken from thread dumps were re-fetched by id and matched exactly (checked: 24/24 identical bodies and timestamps). The draft's 45 message ids and timestamps were verified against the originals: 45/45 found, all timestamps identical. No text was typed or recreated from memory.

Message timestamps are UTC from the original message. Date range: 2026-10-22 to 2026-11-14.

| group | id | source_message_id | created_at | where | chars | sha256 (text) |
|---|---|---|---|---|---|---|
| primary | `XvJZK8Koo9` | `cmsg_019PsCsHidK7tXiLFjz7RUAKD6mYVjZuBPjBXvJZK8Koo9` | 2026-10-22T17:15:43Z | project chat | 898 | `b015b7dfc314693f…` |
| primary | `QeQzLM8MFd` | `cmsg_019PsCsHidK7tXiLFjz7RUAK7woJRqkw7gs7QeQzLM8MFd` | 2026-10-22T17:47:03Z | thread eR7fMx | 677 | `b2ec89163ddc905c…` |
| primary | `9VgNem1Ftm` | `cmsg_019PsCsHidK7tXiLFjz7RUAKHbHFY3pdA1Qh9VgNem1Ftm` | 2026-10-23T09:39:13Z | project chat | 761 | `27798d59c639d597…` |
| primary | `CK3sKxD5Yq` | `cmsg_019PsCsHidK7tXiLFjz7RUAK1TvgBnRbkNfRCK3sKxD5Yq` | 2026-10-23T15:58:13Z | project chat | 226 | `bea7a27c459b6f4f…` |
| primary | `mr7DwLWcwE` | `cmsg_019PsCsHidK7tXiLFjz7RUAKGUNi11TZZVVRmr7DwLWcwE` | 2026-10-23T16:01:15Z | thread AbuurA | 130 | `39891f84d9231735…` |
| primary | `SpHCk8xVs9` | `cmsg_019PsCsHidK7tXiLFjz7RUAK1CKhxiXBLCStSpHCk8xVs9` | 2026-10-23T16:43:01Z | thread 21aDg5 | 125 | `c8e7b05a91280290…` |
| primary | `DpQd1dfZwF` | `cmsg_019PsCsHidK7tXiLFjz7RUAK3ZmysbsXzYKsDpQd1dfZwF` | 2026-10-23T17:16:49Z | thread 21aDg5 | 163 | `845d15dee923ae76…` |
| primary | `4QtyBgvypt` | `cmsg_019PsCsHidK7tXiLFjz7RUAKVvbqYMB8uKn34QtyBgvypt` | 2026-10-23T17:21:25Z | thread 21aDg5 | 88 | `976e52548b44ea11…` |
| primary | `pzG2mTog1s` | `cmsg_019PsCsHidK7tXiLFjz7RUAK6woxcTit8rKLpzG2mTog1s` | 2026-10-24T13:42:13Z | thread 21aDg5 | 627 | `8f86295fe4b7eff6…` |
| primary | `a4NXvQCcJx` | `cmsg_019PsCsHidK7tXiLFjz7RUAK9hYisBz22HXha4NXvQCcJx` | 2026-10-24T16:55:37Z | project chat | 626 | `283e558f6bc51f30…` |
| primary | `F4LqFDKhEV` | `cmsg_019PsCsHidK7tXiLFjz7RUAK5jqatxnXmihEF4LqFDKhEV` | 2026-10-24T17:53:14Z | thread 21aDg5 | 3232 | `f1dd017576c4115b…` |
| primary | `aeykVkqsM1` | `cmsg_019PsCsHidK7tXiLFjz7RUAKXcZgwmEaSaBFaeykVkqsM1` | 2026-10-25T00:38:34Z | thread 21aDg5 | 2503 | `8172418923bfe2a1…` |
| primary | `W7gZj66JGc` | `cmsg_019PsCsHidK7tXiLFjz7RUAKMmX2xZ8fgJSPW7gZj66JGc` | 2026-10-25T00:39:57Z | thread 21aDg5 | 85 | `1970e1b53ac7b89d…` |
| primary | `34gjvHeyGQ` | `cmsg_019PsCsHidK7tXiLFjz7RUAKA9YePQxxCiZm34gjvHeyGQ` | 2026-10-25T02:26:01Z | project chat | 776 | `346ac61cf541781a…` |
| primary | `Dix3H6Fyph` | `cmsg_019PsCsHidK7tXiLFjz7RUAKXmBUB2h3Zf1uDix3H6Fyph` | 2026-10-29T15:04:39Z | thread rXL5Pf | 398 | `d7ea6f30259fbe16…` |
| primary | `RjVowB8ciP` | `cmsg_019PsCsHidK7tXiLFjz7RUAK8Wjr4edpQdthRjVowB8ciP` | 2026-10-29T15:27:10Z | thread rXL5Pf | 187 | `3fb55b5fa8452a6e…` |
| primary | `c4dBkhh14Z` | `cmsg_019PsCsHidK7tXiLFjz7RUAKAGdhavQNCmY5c4dBkhh14Z` | 2026-10-29T18:54:18Z | project chat | 214 | `f16e61fa50ee1bb5…` |
| primary | `dNEK4oF7yV` | `cmsg_019PsCsHidK7tXiLFjz7RUAKRPYT8dRPbrXTdNEK4oF7yV` | 2026-10-30T13:28:11Z | project chat | 1482 | `2f8bf41a4b043c0e…` |
| primary | `VQWiPDEgbm` | `cmsg_019PsCsHidK7tXiLFjz7RUAKMXhKZ6ZcGVSgVQWiPDEgbm` | 2026-10-30T17:24:15Z | thread 4oF7yV | 5328 | `6110e8a828c2b594…` |
| primary | `NsYAnzNThU` | `cmsg_019PsCsHidK7tXiLFjz7RUAK9AgMkkDQXQjiNsYAnzNThU` | 2026-10-30T21:05:20Z | thread 4oF7yV | 2601 | `7032b3b81ef2be33…` |
| primary | `wYCfpHgci1` | `cmsg_019PsCsHidK7tXiLFjz7RUAK2KmiDcJ7vdYwwYCfpHgci1` | 2026-10-31T00:36:08Z | project chat | 9460 | `22d58b49dabc70a9…` |
| primary | `BjxMTw3GNE` | `cmsg_019PsCsHidK7tXiLFjz7RUAKG1BLtvvAXQZHBjxMTw3GNE` | 2026-10-31T01:31:10Z | thread 4oF7yV | 19567 | `9886bdfff7862141…` |
| primary | `xhMcczbtWd` | `cmsg_019PsCsHidK7tXiLFjz7RUAK3Ss1FEJ752MzxhMcczbtWd` | 2026-11-06T18:56:12Z | project chat | 8267 | `beee6b02eba9c14b…` |
| primary | `yrQdLMvh4s` | `cmsg_019PsCsHidK7tXiLFjz7RUAKVbmkEJ4HeufcyrQdLMvh4s` | 2026-11-10T01:43:33Z | project chat | 1149 | `53ccd1e5f8954424…` |
| primary | `pEjbaBnj1M` | `cmsg_019PsCsHidK7tXiLFjz7RUAKSrXVdHLd5uTkpEjbaBnj1M` | 2026-11-10T02:02:25Z | thread XXjB2S | 1177 | `0d2f629143739f4f…` |
| primary | `3rC3iSEhvD` | `cmsg_019PsCsHidK7tXiLFjz7RUAKFjCR98PY1wrW3rC3iSEhvD` | 2026-11-10T02:55:44Z | project chat | 291 | `046bdf84ef496f1f…` |
| primary | `aH7hF8sRkG` | `cmsg_019PsCsHidK7tXiLFjz7RUAKHsvJ3XK5RNNTaH7hF8sRkG` | 2026-11-12T13:07:06Z | project chat | 994 | `347707a13dc7d482…` |
| primary | `K1mHACVkPQ` | `cmsg_019PsCsHidK7tXiLFjz7RUAKCCzoW8JZa62mK1mHACVkPQ` | 2026-11-12T13:39:50Z | thread F8sRkG | 590 | `73b74f7387d48fe2…` |
| primary | `bf7eL9rznC` | `cmsg_019PsCsHidK7tXiLFjz7RUAKArSZzzB3AzQibf7eL9rznC` | 2026-11-12T18:31:23Z | thread F8sRkG | 63 | `70c6792f0da71584…` |
| primary | `ZyixXbQTvw` | `cmsg_019PsCsHidK7tXiLFjz7RUAKTEXoNGc7ayMEZyixXbQTvw` | 2026-11-14T01:13:51Z | thread G8D7zd | 892 | `2448ff837c6d558b…` |
| primary | `MGRARd1VKL` | `cmsg_019PsCsHidK7tXiLFjz7RUAKRV5vJQx7awzwMGRARd1VKL` | 2026-11-14T01:15:18Z | thread AbuurA | 135 | `6b775fcee610d330…` |
| pool | `umnF2qkesp` | `cmsg_019PsCsHidK7tXiLFjz7RUAK2W8DJXa5hUcbumnF2qkesp` | 2026-10-30T17:30:28Z | thread 4oF7yV | 1416 | `3570f8a0599773dc…` |
| pool | `izLnjF4TnL` | `cmsg_019PsCsHidK7tXiLFjz7RUAKDRa17Dn3zQ4vizLnjF4TnL` | 2026-10-30T19:09:03Z | thread 4oF7yV | 429 | `5f94919f56734674…` |
| pool | `ijp71Z5AZt` | `cmsg_019PsCsHidK7tXiLFjz7RUAKVBzN3Lnzv1Ydijp71Z5AZt` | 2026-10-31T14:53:06Z | thread 4oF7yV | 9126 | `06faa44923c51b41…` |
| pool | `9o9f3tCf4u` | `cmsg_019PsCsHidK7tXiLFjz7RUAKBevGaBWPMVLf9o9f3tCf4u` | 2026-11-12T11:29:05Z | project chat | 161 | `6866e3cfe2afd474…` |
| pool | `UANSZCrSdP` | `cmsg_019PsCsHidK7tXiLFjz7RUAKPgVTEMN8CNLbUANSZCrSdP` | 2026-11-12T18:14:43Z | thread F8sRkG | 321 | `1ca1bcbaa284b904…` |
| pool | `x7JWmQtsnD` | `cmsg_019PsCsHidK7tXiLFjz7RUAKGR5q5Ah4sgRSx7JWmQtsnD` | 2026-10-29T14:52:23Z | project chat | 341 | `877b388827f66c5c…` |
| pool | `YdeMLtxPLj` | `cmsg_019PsCsHidK7tXiLFjz7RUAK9Ny5Yr2HeX3CYdeMLtxPLj` | 2026-10-29T15:34:15Z | thread rXL5Pf | 39 | `301ddcc0ad73ad08…` |
| pool | `MHgLPqGHzr` | `cmsg_019PsCsHidK7tXiLFjz7RUAKJNMqQxsVCB2eMHgLPqGHzr` | 2026-11-05T18:26:24Z | project chat | 389 | `be29254b6e17150d…` |
| pool | `SrJFhTmoT9` | `cmsg_019PsCsHidK7tXiLFjz7RUAK4dNUpMnaW9nzSrJFhTmoT9` | 2026-11-05T19:03:44Z | thread PqGHzr | 85 | `6d644bd90432be03…` |
| pool | `txK4nC7PZi` | `cmsg_019PsCsHidK7tXiLFjz7RUAKRvRrToW2M3PctxK4nC7PZi` | 2026-11-10T02:36:04Z | thread XXjB2S | 65 | `d6193cedaa82384c…` |
| pool | `GtKvSDHfQB` | `cmsg_019PsCsHidK7tXiLFjz7RUAK4Tr6MbjkGtZXGtKvSDHfQB` | 2026-11-10T02:55:06Z | thread XXjB2S | 667 | `7bbbd2b7400c4070…` |
| pool | `at7XCYUxdG` | `cmsg_019PsCsHidK7tXiLFjz7RUAK8kgyGijgT8cLat7XCYUxdG` | 2026-11-10T07:22:43Z | thread XXjB2S | 1311 | `fb043838a7165399…` |
| pool | `Ke2VfK4qip` | `cmsg_019PsCsHidK7tXiLFjz7RUAKTtP2hjyT1i5MKe2VfK4qip` | 2026-11-10T14:35:20Z | thread XXjB2S | 477 | `b711cc296a4d0357…` |
| control | `VTsRosZxiV` | `cmsg_019PsCsHidK7tXiLFjz7RUAKDtgsyqTMJ9DVVTsRosZxiV` | 2026-10-23T15:59:02Z | thread AbuurA | 97 | `ef53444e00836a48…` |
| control | `fBvHnrzpqd` | `cmsg_019PsCsHidK7tXiLFjz7RUAKN3SdA9TfPUTQfBvHnrzpqd` | 2026-10-25T01:14:34Z | thread 21aDg5 | 5 | `26b84b5cb13e33e3…` |
| control | `X5c2rXL5Pf` | `cmsg_019PsCsHidK7tXiLFjz7RUAKQHrSFho29K6JX5c2rXL5Pf` | 2026-10-29T13:50:59Z | project chat | 8 | `e256ee8e7aff6957…` |
| control | `d6YKMH8sFa` | `cmsg_019PsCsHidK7tXiLFjz7RUAK3ssjhKUXXkpEd6YKMH8sFa` | 2026-10-29T14:33:39Z | project chat | 13 | `4fbf657cdfadb489…` |
| control | `d4MEWNBX8V` | `cmsg_019PsCsHidK7tXiLFjz7RUAKT4VgDkKBERsrd4MEWNBX8V` | 2026-10-30T17:47:16Z | thread 4oF7yV | 707 | `1b952a1a9859c053…` |
| control | `3vfSNZZAcn` | `cmsg_019PsCsHidK7tXiLFjz7RUAKTSThYGCtW6p83vfSNZZAcn` | 2026-10-30T18:29:18Z | thread 4oF7yV | 2044 | `04f51091c6165d9c…` |
| control | `NEDNECoMJD` | `cmsg_019PsCsHidK7tXiLFjz7RUAKD8h5nMLvFZRsNEDNECoMJD` | 2026-11-06T00:07:50Z | project chat | 6217 | `381862d5d6a2bb03…` |
| control | `BUau1Gzo6o` | `cmsg_019PsCsHidK7tXiLFjz7RUAKHUxCJSz8t95rBUau1Gzo6o` | 2026-11-13T01:07:09Z | project chat | 65 | `683666a5d61d55cb…` |
| control | `a3TMGNAEHP` | `cmsg_019PsCsHidK7tXiLFjz7RUAKKQ3eHqWVRK4Ma3TMGNAEHP` | 2026-11-13T01:08:11Z | project chat | 29 | `12626119ec297c19…` |
| control | `7ktDWmLsxc` | `cmsg_019PsCsHidK7tXiLFjz7RUAKMsfXiLWMs32D7ktDWmLsxc` | 2026-11-14T02:49:20Z | thread G8D7zd | 610 | `695d8e4e6429ca9f…` |
| stress | `MFoEqVv29C` | `cmsg_019PsCsHidK7tXiLFjz7RUAK92dEaPMJzxkBMFoEqVv29C` | 2026-10-29T13:49:15Z | project chat | 26183 | `0509e9816d180ee1…` |

Not used: `Nfb6iD2WFM` (a continuation fragment) and the draft's other "also not tasks" were handled as controls only where listed above.
Limits: single author; all prompts predate Cauldron's runtime; labels are mine. Messages from earlier than the M-series are about Workbench's UI more than about tasks a router will face.
