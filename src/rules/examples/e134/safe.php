<?php
// Safe: validated mail recipient
if (filter_var("static@example.com", FILTER_VALIDATE_EMAIL) !== false) {
    mail("static@example.com", "Welcome", "body");
}
