<div align="center">
    <img src="logo.png" width="200" height="60"/><br>
</div>

___

<div align="center">
    <b>Karutin</b> is a experimental coroutine crate that performs its own code lowering, <ins>without relying on async/await</ins>.
</div>

<!-- ```rust
karutin! {
	pub fn chars(string: &String) -> char..usize {
		for mut ch in string.chars() {
			yield ch;
		}
		string.len()
	}
}
```
