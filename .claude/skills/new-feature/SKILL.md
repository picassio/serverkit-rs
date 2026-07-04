---
name: new-feature
description: Scaffold a new full-stack feature for ServerKit with Flask backend (model, service, API blueprint) and React frontend (page, styles, route). Use when creating new modules, pages, or API endpoints.
disable-model-invocation: true
argument-hint: "[feature-name]"
---

Scaffold a new full-stack feature named **$ARGUMENTS** for ServerKit.

Follow these steps in order:

## 1. Backend Model

Create `backend/app/models/$ARGUMENTS.py` with:
- SQLAlchemy model class (PascalCase name)
- Standard columns: `id`, `created_at`, `updated_at`
- A `to_dict()` method returning JSON-serializable data
- Follow existing model patterns in `backend/app/models/`

## 2. Backend Service

Create `backend/app/services/${ARGUMENTS}_service.py` with:
- Stateless module functions (no classes unless stateful)
- CRUD operations: `create_*`, `get_*`, `get_all_*`, `update_*`, `delete_*`
- Consistent error handling returning `{'error': 'message'}, status_code`
- Import and use the model from step 1

## 3. Backend API Blueprint

Create `backend/app/api/$ARGUMENTS.py` with:
- Flask Blueprint named `${ARGUMENTS}_bp`
- RESTful routes prefixed with the feature name
- All routes decorated with `@jwt_required()`
- JSON request parsing and response formatting
- Call service functions from step 2

Then register the blueprint in `backend/app/__init__.py`:
```python
from app.api.$ARGUMENTS import ${ARGUMENTS}_bp
app.register_blueprint(${ARGUMENTS}_bp, url_prefix='/api/v1/$ARGUMENTS')
```

## 4. Frontend API Methods

Add methods to `frontend/src/services/api.js` in the `ApiService` class:
- `get$ARGUMENTS()`, `create$ARGUMENTS()`, `update$ARGUMENTS()`, `delete$ARGUMENTS()`
- Follow the existing pattern of other API methods in that file

## 5. Frontend Page

Create `frontend/src/pages/$ARGUMENTS.jsx` with:
- Functional component using React hooks
- Fetch data on mount using the API methods from step 4
- Basic CRUD UI with table/list and form
- Loading states and error handling
- Import the LESS stylesheet from step 6

Then add the route in `frontend/src/App.jsx` inside the authenticated routes.

## 6. Frontend Styles

Create `frontend/src/styles/pages/$ARGUMENTS.less` with:
- BEM-like naming: `.${ARGUMENTS}__element--modifier`
- Use existing design system variables (`@card-bg`, `@primary-color`, `@spacing-md`, etc.)
- Import the new file in `frontend/src/styles/main.less`

## Rules

- Python: PEP 8, type hints where helpful
- React: Functional components with hooks only, PascalCase filenames
- LESS: No inline styles, use design system variables
- One logical change per commit
