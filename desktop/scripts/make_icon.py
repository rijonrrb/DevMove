from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

root = Path(__file__).resolve().parent.parent
image = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
d = ImageDraw.Draw(image)
d.rounded_rectangle((54, 54, 970, 970), radius=194, fill="#17283B")
d.rounded_rectangle((118, 153, 164, 872), radius=22, fill="#4B9BD5")
font_path = Path(r"C:\Windows\Fonts\segoeuib.ttf")
font = ImageFont.truetype(str(font_path), 610)
d.text((255, 132), "D", font=font, fill="#F8FBFF", stroke_width=1)
d.ellipse((754, 720, 835, 801), fill="#74C7FD")
output = root / "app-icon.png"
image.save(output)
print(output)
