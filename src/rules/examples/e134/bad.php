<?php
// Bad: mail injection via mail
$to = $_GET['to'] ?? '';
mail($to, "Subject", $body);
