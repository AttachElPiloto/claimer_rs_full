from PIL import Image, ImageDraw, ImageFont
import requests
from io import BytesIO
import os
import sys

CLIENT_ID = "006401f1285e9c7"  # ← Remplace ici

def flatten_frame(frame: Image.Image, background=(255, 255, 255)) -> Image.Image:
    flat = Image.new("RGB", frame.size, background)
    flat.paste(frame, mask=frame.split()[3])
    return flat.convert("RGBA")

def generate_gif_with_text(username: str, output_path: str, font_path: str):
    gif_url = "https://i.pinimg.com/originals/7a/97/6d/7a976d8749e34fad7f34d93a44c42d8a.gif"
    gif_data = requests.get(gif_url).content
    base = Image.open(BytesIO(gif_data))

    frames, durations = [], []
    font = ImageFont.truetype(font_path, 70)

    try:
        while True:
            frame = flatten_frame(base.convert("RGBA"))
            draw = ImageDraw.Draw(frame)
            bbox = draw.textbbox((0, 0), username, font=font)
            text_width = bbox[2] - bbox[0]
            x = (frame.width - text_width) // 2
            y = frame.height - 160
            draw.text((x + 2, y + 2), username, font=font, fill=(0, 0, 0, 128))
            draw.text((x, y), username, font=font, fill=(255, 255, 255, 255))
            frames.append(frame.convert("P", palette=Image.ADAPTIVE))
            durations.append(base.info.get('duration', 100))
            base.seek(base.tell() + 1)
    except EOFError:
        pass

    frames[0].save(
        output_path,
        save_all=True,
        append_images=frames[1:],
        loop=0,
        duration=durations,
        disposal=2,
        transparency=100
    )

def upload_to_imgur(filepath: str, client_id: str) -> str:
    headers = {"Authorization": f"Client-ID {client_id}"}
    with open(filepath, 'rb') as f:
        response = requests.post(
            "https://api.imgur.com/3/image",
            headers=headers,
            files={"image": f},
            verify=False
        )
    if response.status_code == 200:
        return response.json()["data"]["link"]
    else:
        raise Exception(f"❌ Imgur upload failed: {response.status_code} - {response.text}")

if __name__ == "__main__":
    username = sys.argv[1] if len(sys.argv) > 1 else "Anonymous"
    os.makedirs("gif_output", exist_ok=True)
    output_path = f"gif_output/{username.lower()}.gif"

    generate_gif_with_text(
        username=username,
        output_path=output_path,
        font_path="src/utilities/assets/Starbim.ttf"
    )

    url = upload_to_imgur(output_path, CLIENT_ID)
    with open("gif_output/link.txt", "w") as f:
        f.write(url)
    os.remove(output_path)
    