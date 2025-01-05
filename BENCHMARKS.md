🔬 Vector Store Benchmark Suite
============================

╔═══════════════════════════════════════════════════════════════╗
║ 🚀 Memory Operations                                           ║
╚═══════════════════════════════════════════════════════════════╝
⠁ [00:00:00] [---------------------------------------------------------------------------------------------------] 0/100 (0s)Benchmarking memory_ops_bert-base/search_small_chat_histo
memory_ops_bert-base/search_small_chat_history_ExactMatch
                        time:   [68.645 µs 69.806 µs 70.307 µs]
                        change: [+7.8845% +9.6156% +11.368%] (p = 0.00 < 0.05)
                        Performance has regressed.
memory_ops_bert-base/search_small_chat_history_Semantic
                        time:   [1.1702 ms 1.1957 ms 1.2454 ms]
                        change: [+0.7468% +4.5684% +8.7786%] (p = 0.04 < 0.05)
                        Change within noise threshold.
Found 3 outliers among 10 measurements (30.00%)
  1 (10.00%) low mild
  1 (10.00%) high mild
  1 (10.00%) high severe
memory_ops_bert-base/search_small_chat_history_Hybrid
                        time:   [516.47 µs 520.86 µs 525.03 µs]
                        change: [-14.224% -11.773% -9.3776%] (p = 0.00 < 0.05)
                        Performance has improved.
memory_ops_bert-base/search_small_knowledge_base_ExactMatch
                        time:   [59.363 µs 59.556 µs 59.768 µs]
                        change: [-23.166% -22.698% -22.153%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 1 outliers among 10 measurements (10.00%)
  1 (10.00%) high severe
memory_ops_bert-base/search_small_knowledge_base_Semantic
                        time:   [757.52 µs 760.48 µs 764.10 µs]
                        change: [-14.150% -12.862% -11.573%] (p = 0.00 < 0.05)
                        Performance has improved.
memory_ops_bert-base/search_small_knowledge_base_Hybrid
                        time:   [395.36 µs 397.51 µs 400.34 µs]
                        change: [-18.585% -17.169% -15.659%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 1 outliers among 10 measurements (10.00%)
  1 (10.00%) high severe
memory_ops_bert-base/search_small_mixed_ExactMatch
                        time:   [104.87 µs 105.10 µs 105.39 µs]
                        change: [+44.064% +45.559% +47.142%] (p = 0.00 < 0.05)
                        Performance has regressed.
Found 2 outliers among 10 measurements (20.00%)
  2 (20.00%) high severe
memory_ops_bert-base/search_small_mixed_Semantic
                        time:   [784.98 µs 798.54 µs 811.86 µs]
                        change: [-13.963% -9.2307% -4.4250%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 1 outliers among 10 measurements (10.00%)
  1 (10.00%) high mild
memory_ops_bert-base/search_small_mixed_Hybrid
                        time:   [436.04 µs 437.48 µs 440.12 µs]
                        change: [-1.2644% +1.5354% +4.6873%] (p = 0.34 > 0.05)
                        No change in performance detected.
Found 2 outliers among 10 measurements (20.00%)
  2 (20.00%) high severe
 setting number of points 50000 
 setting number of points 100000 
memory_ops_bert-base/search_medium_chat_history_ExactMatch
                        time:   [278.78 µs 279.19 µs 279.64 µs]
                        change: [-32.832% -32.130% -31.547%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 1 outliers among 10 measurements (10.00%)
  1 (10.00%) high severe
memory_ops_bert-base/search_medium_chat_history_Semantic
                        time:   [3.4916 ms 3.5033 ms 3.5226 ms]
                        change: [-7.2538% -5.4991% -3.9252%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 3 outliers among 10 measurements (30.00%)
  1 (10.00%) low mild
  1 (10.00%) high mild
  1 (10.00%) high severe
memory_ops_bert-base/search_medium_chat_history_Hybrid
                        time:   [1.7735 ms 1.7880 ms 1.8003 ms]
                        change: [-8.7938% -6.3942% -4.2965%] (p = 0.00 < 0.05)
                        Performance has improved.
 setting number of points 150000 
 setting number of points 200000 
memory_ops_bert-base/search_medium_knowledge_base_ExactMatch
                        time:   [443.08 µs 443.78 µs 444.68 µs]
                        change: [+7.9230% +9.8638% +11.647%] (p = 0.00 < 0.05)
                        Performance has regressed.
Found 1 outliers among 10 measurements (10.00%)
  1 (10.00%) high severe
memory_ops_bert-base/search_medium_knowledge_base_Semantic
                        time:   [5.2470 ms 5.2815 ms 5.3106 ms]
                        change: [-2.7520% -0.9817% +0.9746%] (p = 0.33 > 0.05)
                        No change in performance detected.
Found 1 outliers among 10 measurements (10.00%)
  1 (10.00%) high mild
memory_ops_bert-base/search_medium_knowledge_base_Hybrid
                        time:   [2.6749 ms 2.7006 ms 2.7285 ms]
                        change: [-3.8456% -1.4718% +0.5244%] (p = 0.26 > 0.05)
                        No change in performance detected.
Found 2 outliers among 10 measurements (20.00%)
  1 (10.00%) low mild
  1 (10.00%) high mild
 setting number of points 250000 
 setting number of points 300000 
memory_ops_bert-base/search_medium_mixed_ExactMatch
                        time:   [658.50 µs 660.14 µs 662.76 µs]
                        change: [-14.386% -11.716% -8.7525%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 1 outliers among 10 measurements (10.00%)
  1 (10.00%) high severe
memory_ops_bert-base/search_medium_mixed_Semantic
                        time:   [5.6537 ms 5.6947 ms 5.7401 ms]
                        change: [-4.3611% -2.0285% +0.1951%] (p = 0.12 > 0.05)
                        No change in performance detected.
memory_ops_bert-base/search_medium_mixed_Hybrid
                        time:   [2.8144 ms 2.8522 ms 2.8823 ms]
                        change: [-5.6997% -4.2669% -2.8747%] (p = 0.00 < 0.05)
                        Performance has improved.
 setting number of points 350000 
 setting number of points 400000 
 setting number of points 450000 
 setting number of points 500000 
 setting number of points 550000 
 setting number of points 600000 
 setting number of points 650000 
 setting number of points 700000 
 setting number of points 750000 
 setting number of points 800000 
 setting number of points 850000 
 setting number of points 900000 
 setting number of points 950000 
 setting number of points 1000000 
 setting number of points 1050000 
 setting number of points 1100000
 setting number of points 1150000 
 setting number of points 1200000
 setting number of points 1250000
