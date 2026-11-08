<?php
// Bad: XPath injection via user-controlled input
$query = $_GET['query'] ?? '';
$xml = simplexml_load_string($query);
$result = simplexml_load_string($_POST['xml']);
