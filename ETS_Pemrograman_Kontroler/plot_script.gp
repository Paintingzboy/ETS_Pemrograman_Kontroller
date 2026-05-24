# 1. Atur pemisah data menggunakan tanda koma (karena file kita .csv)
set datafile separator ","

# 2. Atur format output menjadi gambar PNG berkualitas tinggi
set terminal pngcairo size 800,600 enhanced font "Arial,10"
set output "grafik_latensi_ets.png"

# 3. Atur judul dan label sumbu grafik
set title "Grafik Verifikasi Performas Latensi Kontroler ASMC pada ESP32-S3" font "Arial Bold,12"
set xlabel "Waktu Operasi Sistem (ms)"
set ylabel "Latensi Eksekusi (mikrodetik - us)"

# 4. Atur batas tampilan grafik dan grid agar rapi
set grid
set yrange [0:30]

# 5. Lakukan plot data: Mengambil Kolom 1 sebagai X, dan Kolom 4 sebagai Y
plot "data_asmc.csv" using 1:4 with linespoints linestyle 1 linewidth 2 linecolor rgb "blue" title "Latensi ASMC (C++ Proxy)"