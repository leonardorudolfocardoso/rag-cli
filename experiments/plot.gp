set datafile separator ","

# --- 1. Chunk size sweep (fixed k = 3) ---
set terminal pngcairo size 1200,480 noenhanced font "Helvetica,12" background rgb "#ffffff"
set output "experiments/chunk_size.png"

set multiplot layout 1,2 title "Plaintext RFCs: Chunk Size Sweep (fixed k = 3, 95.6k chars, 20 cases)" font "Helvetica,15"

# Left panel: Recall & MRR
set title "Retrieval Quality (Inverted-U)" font "Helvetica,13"
set xlabel "chunk_size (chars)"
set ylabel "Score (0 to 1)"
set yrange [0:1.08]
set offsets 0.25, 0.25, 0, 0
set grid y lt 0 lw 1 lc rgb "#dddddd"
set key bottom right box opaque
set xtics rotate by 0
plot "experiments/chunk_size.csv" every ::1 using 0:6:xtic(1) with linespoints lw 2.5 pt 7 ps 1.3 lc rgb "#2563eb" title "single@3 recall", \
     "" every ::1 using 0:9 with linespoints lw 2.5 pt 5 ps 1.2 lc rgb "#16a34a" title "overall@3 recall", \
     "" every ::1 using 0:10 with linespoints lw 2.5 pt 9 ps 1.3 lc rgb "#ea580c" title "MRR"

# Right panel: Prompt size
set title "Avg Prompt Context Size (k = 3)" font "Helvetica,13"
set xlabel "chunk_size (chars)"
set ylabel "avg_prompt_chars"
set yrange [0:19500]
set offsets 0, 0, 0, 0
set style fill solid 0.75 border -1
set boxwidth 0.55
set key off
plot "experiments/chunk_size.csv" every ::1 using 0:11:xtic(1) with boxes lc rgb "#6366f1" title "avg_prompt_chars", \
     "" every ::1 using 0:($11+600):(sprintf("%d", $11)) with labels font "Helvetica,9"

unset multiplot

# --- 2. Top-k sweep (fixed chunk_size = 1100) ---
set terminal pngcairo size 1200,480 noenhanced font "Helvetica,12" background rgb "#ffffff"
set output "experiments/top_k.png"

set multiplot layout 1,2 title "Plaintext RFCs: Top-k Sweep (chunk_size = 1100, 105 chunks, 20 cases)" font "Helvetica,15"

# Left panel: Recall & MRR
set title "Retrieval Quality vs k" font "Helvetica,13"
set xlabel "k"
set ylabel "Score (0 to 1)"
set yrange [0:1.08]
set offsets 0.25, 0.25, 0, 0
set grid y lt 0 lw 1 lc rgb "#dddddd"
set key bottom right box opaque
plot "experiments/top_k.csv" every ::1 using 0:6:xtic(2) with linespoints lw 2.5 pt 7 ps 1.3 lc rgb "#2563eb" title "single@k recall", \
     "" every ::1 using 0:12 with linespoints lw 2.5 pt 5 ps 1.2 lc rgb "#16a34a" title "overall@k recall", \
     "" every ::1 using 0:13 with linespoints lw 2.5 pt 9 ps 1.3 lc rgb "#ea580c" title "MRR", \
     "" every ::1 using 0:9 with linespoints lw 2 dt 2 pt 4 ps 1.1 lc rgb "#dc2626" title "cross@k recall"

# Right panel: Prompt size
set title "Avg Prompt Context Size vs k" font "Helvetica,13"
set xlabel "k"
set ylabel "avg_prompt_chars"
set yrange [0:11500]
set offsets 0, 0, 0, 0
set style fill solid 0.75 border -1
set boxwidth 0.55
set key off
plot "experiments/top_k.csv" every ::1 using 0:14:xtic(2) with boxes lc rgb "#6366f1" title "avg_prompt_chars", \
     "" every ::1 using 0:($14+350):(sprintf("%d", $14)) with labels font "Helvetica,9"

unset multiplot
