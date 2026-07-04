"""OpenAPI/Swagger documentation endpoints."""
from flask import Blueprint, jsonify

docs_bp = Blueprint('docs', __name__)


@docs_bp.route('/', methods=['GET'])
def swagger_ui():
    """Serve Swagger UI."""
    html = '''<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>ServerKit API Documentation</title>
    <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
    <style>
        body { margin: 0; padding: 0; }
        .swagger-ui .topbar { display: none; }
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script>
        SwaggerUIBundle({
            url: './openapi.json',
            dom_id: '#swagger-ui',
            presets: [
                SwaggerUIBundle.presets.apis,
                SwaggerUIBundle.SwaggerUIStandalonePreset
            ],
            layout: 'BaseLayout',
            deepLinking: true,
            defaultModelsExpandDepth: -1,
        });
    </script>
</body>
</html>'''
    return html, 200, {'Content-Type': 'text/html'}


@docs_bp.route('/openapi.json', methods=['GET'])
def openapi_spec():
    """Return the generated OpenAPI specification."""
    from app.services.openapi_service import OpenAPIService
    spec = OpenAPIService.generate_spec()
    return jsonify(spec)
