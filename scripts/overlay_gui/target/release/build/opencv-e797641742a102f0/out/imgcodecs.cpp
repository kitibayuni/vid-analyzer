#include "ocvrs_common.hpp"
#include <opencv2/imgcodecs.hpp>
#include "imgcodecs_types.hpp"

extern "C" {
void cv_haveImageReader_const_StringR(const char* filename, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::haveImageReader(std::string(filename));
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_haveImageWriter_const_StringR(const char* filename, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::haveImageWriter(std::string(filename));
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imcount_const_StringR(const char* filename, Result<size_t>* ocvrs_return) {
	try {
		size_t ret = cv::imcount(std::string(filename));
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imcount_const_StringR_int(const char* filename, int flags, Result<size_t>* ocvrs_return) {
	try {
		size_t ret = cv::imcount(std::string(filename), flags);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imdecode_const__InputArrayR_int(const cv::_InputArray* buf, int flags, Result<cv::Mat*>* ocvrs_return) {
	try {
		cv::Mat ret = cv::imdecode(*buf, flags);
		Ok(new cv::Mat(ret), ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imdecode_const__InputArrayR_int_MatX(const cv::_InputArray* buf, int flags, cv::Mat* dst, Result<cv::Mat*>* ocvrs_return) {
	try {
		cv::Mat ret = cv::imdecode(*buf, flags, dst);
		Ok(new cv::Mat(ret), ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imencode_const_StringR_const__InputArrayR_vectorLunsigned_charGR(const char* ext, const cv::_InputArray* img, std::vector<unsigned char>* buf, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::imencode(std::string(ext), *img, *buf);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imencode_const_StringR_const__InputArrayR_vectorLunsigned_charGR_const_vectorLintGR(const char* ext, const cv::_InputArray* img, std::vector<unsigned char>* buf, const std::vector<int>* params, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::imencode(std::string(ext), *img, *buf, *params);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imread_const_StringR(const char* filename, Result<cv::Mat*>* ocvrs_return) {
	try {
		cv::Mat ret = cv::imread(std::string(filename));
		Ok(new cv::Mat(ret), ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imread_const_StringR_int(const char* filename, int flags, Result<cv::Mat*>* ocvrs_return) {
	try {
		cv::Mat ret = cv::imread(std::string(filename), flags);
		Ok(new cv::Mat(ret), ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imreadmulti_const_StringR_vectorLMatGR(const char* filename, std::vector<cv::Mat>* mats, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::imreadmulti(std::string(filename), *mats);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imreadmulti_const_StringR_vectorLMatGR_int(const char* filename, std::vector<cv::Mat>* mats, int flags, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::imreadmulti(std::string(filename), *mats, flags);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imreadmulti_const_StringR_vectorLMatGR_int_int(const char* filename, std::vector<cv::Mat>* mats, int start, int count, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::imreadmulti(std::string(filename), *mats, start, count);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imreadmulti_const_StringR_vectorLMatGR_int_int_int(const char* filename, std::vector<cv::Mat>* mats, int start, int count, int flags, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::imreadmulti(std::string(filename), *mats, start, count, flags);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imwrite_const_StringR_const__InputArrayR(const char* filename, const cv::_InputArray* img, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::imwrite(std::string(filename), *img);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imwrite_const_StringR_const__InputArrayR_const_vectorLintGR(const char* filename, const cv::_InputArray* img, const std::vector<int>* params, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::imwrite(std::string(filename), *img, *params);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imwritemulti_const_StringR_const__InputArrayR(const char* filename, const cv::_InputArray* img, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::imwritemulti(std::string(filename), *img);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_imwritemulti_const_StringR_const__InputArrayR_const_vectorLintGR(const char* filename, const cv::_InputArray* img, const std::vector<int>* params, Result<bool>* ocvrs_return) {
	try {
		bool ret = cv::imwritemulti(std::string(filename), *img, *params);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

}
