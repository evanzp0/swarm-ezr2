FROM python:3.11

WORKDIR /tracker 

ENV DB_PATH=/tracker/db

COPY requirements.txt ./requirements.txt
RUN pip install --no-cache-dir --upgrade -r requirements.txt

COPY tracker/. .

CMD ["gunicorn", "-b", ":6969", "tracker:tracker"]