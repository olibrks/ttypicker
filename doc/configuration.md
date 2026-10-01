# Configuration <!-- omit from toc -->

- [Config File](#config-file)
  - [Keys](#keys)
  - [Overriding by method and application](#overriding-by-method-and-application)
- [Tokens](#tokens)
  - [Tokens as a condition](#tokens-as-a-condition)
  - [Main tokens](#main-tokens)
  - [Tokens from the original FileChooser request](#tokens-from-the-original-filechooser-request)
  - [Tokens created for convenience](#tokens-created-for-convenience)


## Config File

The config file is using TOML syntax to define a `[Filechooser]` table. 

### Keys
<table>
    <tr>
        <td><strong>term</strong></td>
        <td>Required</td>
        <td>terminal part of the command launched by <code>ttypicker</code> (including -e or -- if needed).</td>
    </tr>
    <tr>
        <td><strong>exec</strong></td>
        <td>Required</td>
        <td>TUI file manager part of the command launched by <code>ttypicker</code>.</td>
    </tr>
    <tr>
        <td><strong>in_path</strong></td>
        <td>Required</td>
        <td>starting path for the file manager, to be used as <code>{IN_PATH}</code> in the exec command. Typical values are  or <code>in_path={SUGGESTED_PATH}</code>. In case of <code>SaveFile</code> or <code>SaveFiles</code> methods, it creates a temporary placeholder. If the path already exists, it will be suffixed with <code>_X</code>  (e.g. <code>existing_file_1.txt</code>).
        <br><strong>Standard options:</stromg>
        <ul>
        <li><code>in_path={SUGGESTED_PATH}</code>: Path suggested by the application issuing the file picker request.</li>
        <li><code>in_path={LAST_PATH}</code>: Suggested path in the last folder used.</li>
        <li><code>in_path={APP_LAST_PATH}</code>: Suggested path in the last folder used by the the application issuing the file picker request.</li>
        </ul>
    </tr>
    <tr>
        <td style="white-space: nowrap;"><strong>env.&lt;VAR&gt;</strong></td>
        <td>Optional</td>
        <td>environment variable to create or update with its values. The syntax is: <code>env.ENV_VAR="VALUE"</code> (only the value needs quotes). Use value <code>""</code> to unset an environment variable. Multiple environment variables can be defined.</td>
    </tr>
    <tr>
        <td><strong>default_dir</strong></td>
        <td>Optional</td>
        <td>absolute path to an existing directory to override the default directory <code>/tmp</code>. No token allowed.</td>
    </tr>
</table>

### Overriding by method and application
The keys can also be further refined by method (`OpenFile`, `SaveFile` and `SaveFiles`) and by application (second level) using nested tables. `ttypicker` resolves each key (term, exec, in_path and env) from the leaf table up to the root. For example, the config file could be structured as:
```toml
[FileChooser]
term="kitty --class yazi --title {TITLE}"
exec="yazi --chooser-file={OUT_PATH} {IN_PATH}"
in_path="{SUGGESTED_PATH}"
default_dir="/home/username/Downloads"

[FileChooser.SaveFile]
in_path="{LAST_PATH}"

[FileChooser.SaveFile.firefox]
term="kitty --class yazi --title 'Firefox - Save as...'"
```
In this example, the last path is used for `SaveFile`, while the app suggested path is used for all other methods. Furthermore, firefox app has a specific title when triggering `SaveFile` method.

> [!Note]
> Not all applications are sending an `app_id`, or that `app_id` could be generic. You can look at the `app_id` in the logs.

>[!Caution]
> When creating complex config files, make sure that `term` and `exec` keys are always defined. Similarly, if you are using `{IN_PATH}` token at any level, make sure it's always defined with `in_path` key.

## Tokens
As shown above, tokens can be used in the config file. Most of the tokens are represented as strings. Other tokens represent boolean values, such as `{MULTIPLE}` or `{DIRECTORY}`.

### Tokens as a condition
Boolean tokens can be used as a condition with the `?` pattern: `{?MULTIPLE:<IF_TRUE>}` or `{?MULTIPLE:<IF_TRUE>:<IF_FALSE>}`.
The presence of a token can be checked with  the `#` pattern: `{#TITLE:<IF_PRESENT>:<IF_NOT_PRESENT>}`.

### Main tokens

<table>
    <tbody>
    <tr>
        <td><strong>{OUT_PATH}</strong></td>
        <td>Path to the temporary file where the file chooser writes the selected paths</td>
    </tr>
    <tr>
        <td><strong>{IN_PATH}</strong></td>
        <td>Starting path for the TUI file manager. Defined by <code>in_path=</code> in the <code>config.toml</code> file.</td>
    </tr>
    <tr>
        <td><strong>{SUGGESTED_PATH}</strong></td>
        <td>Path suggested by the application issuing the file picker request.</td>
    </tr>
    <tr>
        <td><strong>{LAST_PATH}</strong></td>
        <td>Suggested path in the last folder used.</td>
    </tr>
    <tr>
        <td><strong>{APP_LAST_PATH}</strong></td>
        <td>Suggested path in the last folder used by the the application issuing the file picker request.</td>
    </tr>
    </tbody>
</table>

### Tokens from the original FileChooser request
<table>
    <thead>
    <tr>
        <th>Token</th>
        <th>OpenFile</th>
        <th>SaveFile</th>
        <th>SaveFiles</th>
    </tr>
    </thead>
    <tbody>
    <tr>
        <td><strong>{METHOD}</strong></td>
        <td colspan="3">Requested D-Bus method.</td>
    </tr>
    <tr>
        <td><strong>{APP_ID}</strong></td>
        <td colspan="3">Application identifier of the app requesting the file picker</td>
    </tr>
    <tr>
        <td><strong>{WINDOW_ID}</strong></td>
        <td colspan="3">Window identifier of the app requesting the file picker</td>
    </tr>
    <tr>
        <td><strong>{TITLE}</strong></td>
        <td colspan="3">Title text for the file chooser dialog</td>
    </tr>
    <tr>
        <td><strong>{ACCEPT_LABEL}</strong></td>
        <td colspan="3">Custom text label for the dialog's "Accept" or "Submit" button</td>
    </tr>
    <tr>
        <td><strong>{MODAL}</strong></td>
        <td colspan="3">Boolean indicating whether the dialog should block the parent window</td>
    </tr>
    <tr>
        <td><strong>{MULTIPLE}</strong></td>
        <td>Indicates whether the user can select multiple files</td>
        <td colspan="2">Not applicable (expects exactly one output path)</td>
    </tr>
    <tr>
        <td><strong>{DIRECTORY}</strong></td>
        <td>Indicates whether the user is selecting a folder instead of a file</td>
        <td>Not applicable</td>
        <td>Not applicable</td>
    </tr>
    <tr>
        <td><strong>{CURRENT_FOLDER}</strong></td>
        <td>Suggested folder from which the files should be opened.</td>
        <td>Suggested folder in which the file should be saved.</td>
        <td>Suggested folder in which the files should be saved.</td>
    </tr>
    <tr>
        <td><strong>{CURRENT_NAME}</strong></td>
        <td>Not applicable</td>
        <td>The specific file name requested by the application</td>
        <td>Not applicable</td>
    </tr>
    <tr>
        <td><strong>{CURRENT_FILE}</strong></td>
        <td>Not applicable</td>
        <td>The specific file path requested by the application to overwrite</td>
        <td>Not applicable</td>
    </tr>
    </tbody>
</table>

### Tokens created for convenience

<table>
    <tbody>
    <tr>
        <td><strong>{IN_PATH_NAME}</strong></td>
        <td>Name of the file or folder of <code>{IN_PATH}</code></td>
    </tr>
    <tr>
        <td><strong>{IN_PATH_DIR}</strong></td>
        <td>parent directory of <code>{IN_PATH}</code></td>
    </tr>
    <tr>
        <td><strong>{LAST_DIR}</strong></td>
        <td>Directory last used by the file picker</td>
    </tr>
    <tr>
        <td><strong>{APP_LAST_DIR}</strong></td>
        <td>Directory last used by the file picker for the current application</td>
    </tr>
    <tr>
        <td><strong>{HOME_DIR}</strong></td>
        <td>The user's home directory</td>
    </tr>
    </tbody>
</table>
