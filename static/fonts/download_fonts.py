import os
import re
import urllib.request

fonts_dir = "/home/aaron/.gemini/antigravity/scratch/media-dashboard/static/fonts"
for css_file in ["inter.css", "icons.css"]:
    path = os.path.join(fonts_dir, css_file)
    with open(path, "r") as f:
        content = f.read()
    
    urls = re.findall(r'url\((https://[^\)]+\.ttf)\)', content)
    for url in urls:
        filename = url.split('/')[-1]
        urllib.request.urlretrieve(url, os.path.join(fonts_dir, filename))
        content = content.replace(url, f"/fonts/{filename}")
        
    with open(path, "w") as f:
        f.write(content)
print("Finished downloading fonts and updating CSS.")
