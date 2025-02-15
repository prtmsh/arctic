from flask import Flask, request
app = Flask(__name__)

@app.route('/api', methods=['POST'])
def handle():
    print(request.json)
    return {'status': 'OK'}

if __name__ == '__main__':
    app.run(port=8000)