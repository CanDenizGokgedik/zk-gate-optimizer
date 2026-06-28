<div align="center">

# zk-gate-optimizer

**Plonky2 sıfır-bilgi devrelerinde kapı stratejilerini evrimsel algoritmayla otomatik optimize et.**

*Automatically optimize gate-implementation strategies in Plonky2 zero-knowledge circuits using evolutionary search.*


<br/>

[🇹🇷 Türkçe](#türkçe) &nbsp;·&nbsp; [🇬🇧 English](#english)

</div>

---

## Türkçe

### ZK Nedir?

Sıfır-bilgi ispatı (Zero-Knowledge Proof), **bir bilginin değerini açıklamadan, o bilgiye sahip olduğunu kanıtlamaktır.** Kriptografinin alt dalıdır.

İki taraf vardır:

- **Prover (Kanıtlayan):** Gizli bilgiyi bilen taraf. İspat üretir.
- **Verifier (Onaylayan):** İspat doğruluğunu kontrol eder. Gizli bilgiyi görmez.

Matematiksel altyapısı polinomlar ve eliptik eğriler üzerine kuruludur. Günümüzde finans, blockchain ölçekleme ve veri gizliliğinin kritik olduğu alanlarda kullanılmaktadır.

---

### Somut Örnek: `x² + y² = z`

> **"x² + y² = z olduğunu kanıtla — ama x ve y'yi açıklama."**

```
x = 3,  y = 4  →  z = 9 + 16 = 25

Verifier sadece z = 25'i görür.
x = 3 ve y = 4 olduğunu hiç bilmez.
Ama ispat geçerli olduğu için sonuca güvenir.
```

Bunu yapabilmek için hesabın bir **devreye (circuit)** dönüştürülmesi gerekir.

---

### Devre (Circuit) Nedir?

Kanıtlanması gereken denklem, ZK sistemleri için **kapılardan (gate) oluşan bir tabloya** çevrilir. Her hesap adımı bir kapıdır:

```
x² + y² = z için devre:

Satır │ İşlem        │ Kapı tipi
──────┼──────────────┼──────────────────
  1   │ x * x = a   │ Multiplication Gate
  2   │ y * y = b   │ Multiplication Gate
  3   │ a + b = z   │ Addition Gate
```

Plonky2 bu tabloyu bir polinom haline getirir ve tüm satırların kurala uyduğunu matematiksel olarak kanıtlar — x ve y'nin değerlerini açıklamadan.

**Tablo ne kadar büyükse, ispat o kadar uzun sürer.**

---

### Sorun: Aynı Hesap, Farklı Maliyet

Mevcut ZK frameworklerinde (Plonky2, Halo2 vb.) denklemler otomatik olarak devreye dönüşmez. **Developer her kapıyı elle seçer.** Aynı hesabı birden fazla yöntemle kodlamak mümkündür:

```
x⁷ hesapla:

Yöntem 0 — exp_u64 gadget:      builder.exp_u64(x, 7)
Yöntem 1 — Naif çarpma:         x*x*x*x*x*x*x          (6 kapı)
Yöntem 2 — Square-and-multiply: x² → x⁴ → x⁶ → x⁷     (4 kapı)
```

**Üçü de aynı sonucu verir — ama maliyetleri çok farklıdır:**

| Yöntem | LDE boyutu | İspat süresi |
|--------|:----------:|:------------:|
| `exp_u64` (varsayılan) | 128 | 58.4 ms |
| Naif çarpma | 256 | ~120 ms |
| **Square-and-multiply** | **64** | **28.4 ms** |

Developer bu seçimi genellikle sezgiyle ya da alışkanlıkla yapar — ve çoğunlukla suboptimal kalır.

---

### Bu Proje Ne Yapıyor?

**Kapı stratejisi seçimini otomatikleştirir.**

Her alt-hesap için hangi yöntemin kullanılacağı bir **kromozom** (gen dizisi) ile kodlanır:

```
4 değer için x⁷ hesapla:

Kromozom = [2, 0, 1, 2]
            ↑  ↑  ↑  ↑
            │  │  │  └── 4. değer → Square-and-multiply
            │  │  └───── 3. değer → Naif çarpma
            │  └──────── 2. değer → exp_u64
            └─────────── 1. değer → Square-and-multiply
```

Genetic Algorithm (GA) bu vektörün en iyi kombinasyonunu akıllıca arar:

```
Nesil  0:  60 rastgele kombinasyon  →  en iyi skor = 47
Nesil 10:  İyi genler çoğaldı      →  en iyi skor = 38
Nesil 25:  Plato                   →  en iyi skor = 31
Nesil 80:  Bitti                   →  SONUÇ: [2, 2, 2, 2]
```

> **Tasarım gereği doğruluk garantisi:** Kromozom yalnızca matematiksel olarak eşdeğer seçenekler arasında seçim yapar. Popülasyondaki her birey her zaman doğru sonucu üretir — optimizer hiçbir zaman yanlış devre üretmez.

---

### Algoritmalar

| Algoritma | Açıklama |
|-----------|----------|
| **Genetic Algorithm (GA)** | Tournament selection (k=3), uniform crossover, point mutation, elitism. Ana optimizer. |
| **NSGA-II** | Çok amaçlı GA. `gate_count` ve `lde_size`'ı aynı anda minimize eder. Pareto frontları bulur. |
| **Hill Climbing** | Hamming-1 komşularını dener, en iyisine atlar. Local optimuma sıkışırsa yeniden başlar. |
| **Greedy** | Genleri sırayla, diğerleri sabitken optimize eder. Gen etkileşimlerini göremez. |
| **Random Search** | Aynı bütçeyle tamamen rastgele örnekleme. Temel baseline. |
| **Uniform Sweep** | Sadece 3 birey: tüm genler 0, tüm genler 1, tüm genler 2. En ucuz baseline. |
| **Exhaustive** | Uzay ≤ 200.000 ise hepsini dener. Ground-truth global optimum. |

---

### Sonuçlar

#### Phase 3 — Polynomial Rewrite Benchmark

Degree 12, 15 bağımsız çalışma. Arama uzayı: `6²⁴ ≈ 4.7 × 10¹⁸`

| Algoritma | Medyan gate | Min | Max | Alan bilgisi? |
|-----------|:-----------:|:---:|:---:|:-------------:|
| Uniform sweep | **3** | 3 | 3 | Evet |
| **GA (tam)** | **4** | 4 | 5 | Hayır |
| GA — yalnızca mutation | 5 | 4 | 6 | Hayır |
| GA — yalnızca crossover | 5 | 4 | 6 | Hayır |
| Random search | 7 | 6 | 8 | Hayır |
| Hill climbing | 8 | 6 | 9 | Hayır |
| Greedy | 9 | 8 | 10 | Hayır |

GA, tüm domain-agnostik yöntemlere karşı **p ≈ 3×10⁻⁵** (paired Wilcoxon) istatistiksel anlamlılıkla üstün. Problem büyüdükçe (degree 22) fark **~%35–40'a** ulaşıyor.

#### Exponentiation Benchmark — `x⁷`

| Konfigürasyon | `lde_size` | İspat süresi |
|---------------|:----------:|:------------:|
| Varsayılan — `exp_u64` | 128 | 58.4 ms |
| **GA'nın bulduğu — square-and-multiply** | **64** | **28.4 ms** |

**2× daha küçük devre, ~2× daha hızlı ispat.** Hiçbir alan bilgisi olmadan otomatik keşfedildi.

---

### Hızlı Başlangıç

```bash
# Tüm stratejilerin aynı sonucu ürettiğini doğrula
cargo run --release -- verify --benchmark exp --num-values 4 --exponent 7

# Strateji başına maliyet raporu
cargo run --release -- inspect --benchmark exp --num-values 10 --exponent 7

# GA'yı baseline'larla karşılaştır → results/convergence.csv
cargo run --release -- compare --benchmark exp --num-values 16 --runs 30

# Phase 3 — rewrite-sequence benchmark
cargo run --release -- compare --benchmark poly --degree 12 --seq-len 24 --runs 15

# Grafikleri oluştur
python analysis/plot.py

# İstatistiksel anlamlılık testleri (Wilcoxon)
python analysis/stats.py

# Ölçekleme çalışması — degree 6'dan 22'ye
python analysis/scaling.py
```

---

## English

### What It Does

Many sub-computations in a PLONKish circuit can be expressed with several **mathematically equivalent** gate encodings — a range check via base-2, base-4, or base-8 limb decomposition; a power `xᵏ` via the `exp_u64` gadget, naive repeated multiplication, or square-and-multiply. Each encoding is *correct* but has a different cost profile (trace length, constraint count, polynomial degree, proof size). Today these choices are made by convention. This project searches the space of choices automatically with a genetic algorithm and validates the result against real proof generation.

See [`docs/DESIGN.md`](docs/DESIGN.md) for the full methodology, landscape analysis, and references.

---

### Key Properties

- **Correct by construction** — every gene selects between equivalent encodings; every individual always computes the correct relation; optimization concerns cost only
- **Cheap fitness** — `gate_count` and `lde_size` read directly from compiled circuit without proving (microseconds vs. 50–100 ms per proof)
- **Memoized oracle** — repeated chromosomes (from crossover or elitism) evaluated for free via `HashMap<Chromosome, Cost>`
- **Honest reporting** — cases where GA does *not* win are reported explicitly: NSGA-II Pareto front collapse, uniform sweep beating GA at every scale
- **Reproducible** — seeded `ChaCha8Rng` throughout; all experiments emit CSVs; `RAYON_NUM_THREADS=1` for timing

---

### Headline Results

**Phase 3 (`poly`) — 15 seeded runs, degree 12, seq-len 24, search space `6²⁴ ≈ 4.7×10¹⁸`:**

| Optimizer | median gates | domain knowledge? |
|-----------|:---:|:---:|
| uniform-strategy sweep | **3** | yes |
| **GA (crossover + mutation)** | **4** | no |
| random search | 7 | no |
| hill climbing | 8 | no |
| greedy coordinate descent | 9 | no |

One-sided paired Wilcoxon: **GA < {random, hill-climbing, greedy} on 15/15 runs, p ≈ 3·10⁻⁵**. The GA's advantage grows with problem size — ~35–40% fewer gates than hill climbing at degree 22.

**Exponentiation (`exp`) — `x⁷`:**

| Configuration | `lde_size` | prover time¹ |
|---|:---:|:---:|
| default (`exp_u64`) | 128 | 58.4 ms |
| **evolved (square-and-multiply)** | **64** | **28.4 ms** |

¹ `RAYON_NUM_THREADS=1`, median of 9 proofs after warm-up.

---

### Quick Start

```bash
cargo run --release -- verify  --benchmark exp  --num-values 4  --exponent 7
cargo run --release -- compare --benchmark exp  --num-values 16 --runs 30
cargo run --release -- compare --benchmark poly --degree 12 --seq-len 24 --runs 15
RAYON_NUM_THREADS=1 cargo run --release -- pareto --benchmark exp --num-values 10
python analysis/plot.py && python analysis/stats.py && python analysis/scaling.py
```

---

### Project Layout

```
src/
  field.rs              Goldilocks / Poseidon type aliases (F, C, D)
  strategy.rs           Chromosome encoding, StrategySpace, diversity metric
  fitness.rs            cost_static (no proof) · cost_real (wall-clock)
  experiment.rs         reproducible CSV-emitting runners
  main.rs               CLI: verify | inspect | compare | pareto
  circuits/
    mod.rs              CircuitFactory trait + Built struct
    range_check.rs      base-{2,4,8} limb decomposition
    exponentiation.rs   exp_u64 vs naive vs square-and-multiply
  ec/
    mod.rs              Oracle trait + StaticOracle (memoized HashMap)
    operators.rs        uniform_crossover · point_mutation
    ga.rs               single-objective GA
    nsga2.rs            NSGA-II (non-dominated sort, crowding distance, hypervolume)
    baselines.rs        random · hill climb · greedy · exhaustive · uniform sweep
  rewrite/
    ir.rs               arithmetic DAG IR — Rc<Expr> structural equality → CSE
    rules.rs            6 non-confluent semantics-preserving rewrite rules
    factory.rs          PolyRewrite circuit factory (Phase 3)
tests/
  integration.rs        correctness · GA-vs-exhaustive · GA-beats-hillclimb
analysis/
  plot.py               convergence · diversity · hypervolume plots
  scaling.py            degree sweep 6 → 22
  stats.py              paired Wilcoxon significance tests
docs/
  DESIGN.md             full methodology, landscape analysis, references
presentation/
  build.js              programmatic PPTX generation via pptxgenjs + React
  sunum.pptx            generated Turkish slide deck (11 slides)
```

---

### License

MIT — see [LICENSE](LICENSE).

---

<div align="center">
<sub>Evrimsel Hesaplama &nbsp;·&nbsp; 2026 &nbsp;·&nbsp; Can Deniz Gökgedik</sub>
</div>
