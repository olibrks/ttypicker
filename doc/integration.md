# Integration <!-- omit from toc -->

- [Environment variable](#environment-variable)
- [JSON request file](#json-request-file)


## Environment variable

<table>
    <tbody>
    <tr>
        <td><strong>TTYPICKER_REQ</strong></td>
        <td>Method name: <code>OpenFile</code>, <code>SaveFile</code> or <code>SaveFiles</code></td>
    </tr>
    <tr>
        <td><strong>TTYPICKER_METHOD</strong></td>
        <td>absolute path to the JSON request file</td>
    </tr>
    </tbody>
</table>

## JSON request file

A temporary JSON file provides the original request in the following format: 
```json
{
  "method": "string",
  "app_id": "string",
  "window_id": "string",
  "title": "string",
  "options": {
    "accept_label": "string",
    "modal": true,
    "multiple": false,
    "directory": false,
    "filters": [
      ["name", ["0_or_1", "glob_or_mime"]],
      ...
    ],
    "current_filter": ["string", ["0_or_1", "glob_or_mime"]],
    "choices": [
      ["id", "label", [["id", "label"], ...]],
      ...,
      "initial"
    ],
    "current_name": "name",
    "current_folder": "path",
    "current_file": "path",
    "files": ["uri", ...]
  }
}
```