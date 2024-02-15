import os
from flask import Flask 
from flask import render_template 
  
script_dir = os.path.dirname(__file__)
static_path = f"{script_dir}/../client"
print(static_path)

# creates a Flask application 
app = Flask(__name__,
            static_url_path = "",
            static_folder = static_path)

# run the application 
if __name__ == "__main__": 
    app.run(debug=True)

    
