#!/bin/bash
mkdir -p static/fonts
cd static/fonts

# Download Inter CSS
curl -s -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64)" "https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap" > inter.css

# Extract woff2 URLs and download them
grep -o 'https://[^)]*\.woff2' inter.css | sort -u > inter_urls.txt
while read url; do
    filename=$(basename "$url")
    curl -s "$url" -o "$filename"
    # Replace URL in CSS
    sed -i "s|$url|/fonts/$filename|g" inter.css
done < inter_urls.txt

# Download Material Icons
curl -s -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64)" "https://fonts.googleapis.com/icon?family=Material+Icons" > icons.css

# Extract woff2 URLs and download them
grep -o 'https://[^)]*\.woff2' icons.css | sort -u > icon_urls.txt
while read url; do
    filename="material-icons.woff2" # there's usually just one
    curl -s "$url" -o "$filename"
    sed -i "s|$url|/fonts/$filename|g" icons.css
done < icon_urls.txt

# Clean up
rm inter_urls.txt icon_urls.txt
echo "Done"
