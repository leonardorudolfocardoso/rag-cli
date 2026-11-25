# Run from the repo root: gnuplot experiments/03-chunker/plot.gp
set datafile separator ","

# Plain vs Rfc chunker (chunker.csv: chunker, chunk_size, k, ...) ---
# Rows are filtered with awk: column 1 = chunker, 2 = chunk_size, 3 = k,
# 12 = overall_hits, 15 = mrr, 16 = avg_prompt_chars.
set terminal pngcairo size 1200,480 noenhanced font "Helvetica,12" background rgb "#ffffff"
set output "experiments/03-chunker/chunker.png"

set multiplot layout 1,2 title "Plaintext RFCs: Plain vs Rfc Chunker (20 cases)" font "Helvetica,15"

# Left panel: MRR and overall recall vs chunk_size at k = 3
set title "Retrieval Quality vs chunk_size (k = 3)" font "Helvetica,13"
set xlabel "chunk_size (chars, log scale)"
set ylabel "Score (0 to 1)"
set logscale x
set xrange [170:7000]
set xtics ("200" 200, "400" 400, "800" 800, "1100" 1100, "1500" 1500, "2200" 2200, "3000" 3000, "6000" 6000)
set yrange [0:1.08]
set offsets 0, 0, 0, 0
set grid y lt 0 lw 1 lc rgb "#dddddd"
set key bottom right box opaque
plot "< awk -F, '$1==\"plain\" && $3==3' experiments/03-chunker/chunker.csv" using 2:15 with linespoints lw 2.5 pt 7 ps 1.3 lc rgb "#2563eb" title "Plain MRR", \
     "< awk -F, '$1==\"rfc\" && $3==3' experiments/03-chunker/chunker.csv" using 2:15 with linespoints lw 2.5 pt 9 ps 1.3 lc rgb "#ea580c" title "Rfc MRR", \
     "< awk -F, '$1==\"plain\" && $3==3' experiments/03-chunker/chunker.csv" using 2:14 with linespoints lw 2 dt 2 pt 6 ps 1.1 lc rgb "#2563eb" title "Plain overall@3", \
     "< awk -F, '$1==\"rfc\" && $3==3' experiments/03-chunker/chunker.csv" using 2:14 with linespoints lw 2 dt 2 pt 8 ps 1.1 lc rgb "#ea580c" title "Rfc overall@3"

# Right panel: MRR vs prompt size as k goes 1..10, at each chunker's best chunk_size
set title "MRR vs Prompt Size (k = 1..10)" font "Helvetica,13"
set xlabel "avg_prompt_chars (log scale)"
set ylabel "MRR"
set logscale x
set xrange [800:17000]
set xtics ("1k" 1000, "2k" 2000, "3k" 3000, "5k" 5000, "10k" 10000, "15k" 15000)
set yrange [0.45:0.85]
set key bottom right box opaque
plot "< awk -F, '$1==\"plain\" && $2==1100' experiments/03-chunker/chunker.csv" using 16:15 with linespoints lw 2.5 pt 7 ps 1.2 lc rgb "#2563eb" title "Plain, chunk_size 1100", \
     "< awk -F, '$1==\"rfc\" && $2==1100' experiments/03-chunker/chunker.csv" using 16:15 with linespoints lw 2 dt 2 pt 8 ps 1.1 lc rgb "#ea580c" title "Rfc, chunk_size 1100", \
     "< awk -F, '$1==\"rfc\" && $2==6000' experiments/03-chunker/chunker.csv" using 16:15 with linespoints lw 2.5 pt 9 ps 1.2 lc rgb "#ea580c" title "Rfc, chunk_size 6000", \
     "< awk -F, '$1==\"plain\" && $2==1100' experiments/03-chunker/chunker.csv" using 16:15:(sprintf("k=%d", $3)) every ::0::2 with labels offset 0,-1 font "Helvetica,9" tc rgb "#2563eb" notitle, \
     "< awk -F, '$1==\"rfc\" && $2==6000' experiments/03-chunker/chunker.csv" using 16:15:(sprintf("k=%d", $3)) every ::0::2 with labels offset 0,1 font "Helvetica,9" tc rgb "#ea580c" notitle

unset multiplot
unset logscale x
